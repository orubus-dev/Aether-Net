# AetherNet Physical Underlay

## Purpose

The physical layer carries Aether link frames over the local carrier without making native L2/L3 depend on IP sockets or a specific radio technology.

## Driver boundary

`PhysicalAdapter` describes carrier identity, capabilities and link discovery. `PhysicalDriver` adds bounded frame I/O and an optional carrier analytics surface:

- `mtu()` exposes the maximum Aether payload accepted by the carrier.
- `transmit(link_id, frame)` sends one already-formed Aether link frame.
- `poll_receive()` returns one received Aether frame at a time.
- `analytics(link_id)` returns kernel/device counters when the platform exposes them.

The physical driver never interprets native L3 addresses or L4 flows.

## Runtime

`PhysicalRuntime` owns bounded driver instances and is responsible for driver selection by the discovered `PhysicalLinkId`, not merely by transport class. This permits multiple Ethernet or Wi-Fi interfaces to coexist without one interface stealing another's traffic.

Discovery replaces the validated physical-link table from the currently registered drivers. Inactive links cannot transmit. Transmit is MTU-checked before driver entry, and receive polling is explicitly budgeted so one slow or busy carrier cannot create an unbounded control loop.

## Linux reference implementation

Linux is the reference physical substrate. `LinuxUnderlay` reads `/sys/class/net` and exposes normalized interface name, ifindex, MTU, operational state, transport class, speed and kernel packet/byte/error/drop analytics. Loopback is excluded.

`PhysicalRuntime::discover_linux()` automatically binds active Ethernet and Wi-Fi interfaces. `LinuxEthernetDriver` uses Linux `AF_PACKET` raw sockets and Aether's dedicated EtherType (`0xA37E`). The same driver contract is used for Ethernet and associated Wi-Fi netdevices because Linux exposes their connected data path as Ethernet frames at the packet-socket boundary.

The driver refreshes operational state during discovery so cable/link loss becomes an inactive `PhysicalLink` rather than remaining permanently routable. Driver file descriptors are nonblocking and close-on-exec, and ownership is RAII-managed by Rust.

## Safety and resource bounds

All driver registrations, discovered links and receive polling are bounded. Frame sizes are checked against carrier MTU before transmission and again at the driver boundary. Unknown or inactive links cannot transmit. Linux receive buffers are allocated from a fixed MTU-derived upper bound. Interface and kernel-counter parse failures become typed `PhysicalError` values rather than panics.

`LoopbackDriver` remains an in-memory conformance/emulation driver and is never treated as a hardware transport.

## Platform model

The physical boundary is platform-neutral. Linux has the reference raw-frame implementation. Windows, Android and other platforms bind native networking/device APIs to the same `PhysicalDriver` contract as their kernel-level support is added; unsupported transports return `PhysicalError::Unsupported` rather than silently falling back to an IP-based implementation.

Future direct-radio transports such as Wi-Fi Direct/Aware, Bluetooth LE, UWB, LoRa, cellular and satellite can therefore be added without changing Aether L3/L4 semantics.
