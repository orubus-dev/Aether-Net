# AetherNet Physical Underlay

## Purpose

The physical layer carries Aether link frames over whatever local carrier is available. Native L2/L3 must not depend on an IP socket or a specific radio technology.

## Driver boundary

`PhysicalAdapter` describes carrier identity, capabilities and discovery. `PhysicalDriver` adds the operations required by the runtime:

- `mtu()` exposes the maximum frame size accepted by the carrier.
- `transmit(link_id, frame)` sends one already-formed Aether link frame.
- `poll_receive()` returns one received Aether frame at a time.

The driver never interprets native L3 addresses or transport flows.

## Runtime

`PhysicalRuntime` owns bounded driver instances. Discovery queries every registered driver and replaces the engine's current link table with the validated set of discovered links. Transmission selects the driver associated with the discovered link's transport and validates the frame against the driver's MTU before sending.

Receive polling returns raw Aether link frames to the upper runtime, where authenticated link processing and native L3/L4 handling take over.

## Safety properties

Frame size is bounded by carrier MTU, inactive/unknown links cannot transmit, receive queues are bounded by the driver implementation, and link discovery rejects invalid bandwidth, signal-quality and packet-loss values.

`LoopbackDriver` is intentionally an in-memory implementation for conformance tests and emulation. It is not presented as a hardware driver.

## Platform integration

OS-specific implementations will bind the same `PhysicalDriver` contract to Ethernet interfaces, Wi-Fi, Wi-Fi Direct/Aware, Bluetooth LE, UWB and future LoRa/cellular/satellite transports. Those bindings belong outside the carrier-neutral protocol logic.
