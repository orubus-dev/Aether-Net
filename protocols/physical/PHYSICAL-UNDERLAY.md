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

## Linux integration

Linux is the reference physical substrate. `LinuxUnderlay` reads kernel-visible interfaces from `/sys/class/net`, derives normalized MTU/state/speed properties, and exposes the kernel's packet/byte/error/drop counters directly for analytics. Loopback is excluded from physical-carrier discovery.

`PhysicalRuntime::discover_linux()` can attach Linux raw-packet drivers automatically for Ethernet-compatible interfaces. `LinuxEthernetDriver` uses an `AF_PACKET` socket and Aether's dedicated EtherType (`0xA37E`) so the userspace layer does not reimplement IP routing or a second transport stack.

Wi-Fi interfaces are discovered and classified by the Linux underlay. A dedicated nl80211/802.11 data-path driver can be added later without changing the carrier-neutral `PhysicalDriver` contract.

## Safety properties

Frame size is bounded by carrier MTU, inactive/unknown links cannot transmit, receive queues are bounded by the driver implementation, and link discovery rejects invalid bandwidth, signal-quality and packet-loss values.

`LoopbackDriver` is intentionally an in-memory implementation for conformance tests and emulation. It is not presented as a hardware driver.

## Platform integration

OS-specific implementations bind the same `PhysicalDriver` contract to Ethernet, Wi-Fi, Wi-Fi Direct/Aware, Bluetooth LE, UWB and future LoRa/cellular/satellite transports. Linux-specific bindings live beside the carrier-neutral protocol logic rather than leaking OS details into L3/L4.
