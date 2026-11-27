# Network sources, interfaces and attribution

[简体中文](../zh-CN/architecture/NETWORK-SOURCES.md) · [Observer](../../kernel/src/net/mod.rs) · [Packet ledger](../../core/src/network.rs) · [Plan 011](../roadmap/PLAN.md#pr-011)

Plan 011 observes network interfaces and TCP/UDP endpoints from user mode, attributes direction, and keeps raw packets in memory. `CatalogHost` still returns `image_absent`. This plan does not load `strata-net.sys`, call the Service Control Manager, or change driver signing, Secure Boot or HVCI. Observation is `Observer::open` and `Observer::collect`. It does not go through `Manager::subscribe`.

## Interfaces

Linux reads `getifaddrs` and `/sys/class/net`. The kind comes from the interface type, the bridge directory, `IFF_LOOPBACK` and `IFF_POINTOPOINT`. A name prefix is not a kind. `admin_up` follows `IFF_UP`. `operational` is `up`, `down`, `lowerlayerdown`, or unknown when `operstate` is missing.

Windows reads `GetAdaptersAddresses`. Loopback is interface type 24, Ethernet 6, wireless 71, tunnel 131, PPP 23 and proprietary virtual 53. A MAC address is recorded only when `PhysicalAddressLength` is 6. Windows does not expose a separate administrative-up flag, so `admin_up` is true only when `IfOperStatusUp`. Down and lower-layer-down are operational `false`. Other states stay unknown.

## Endpoints and direction

Linux reads `/proc/net/tcp`, `tcp6`, `udp` and `udp6`. A process id comes from a `/proc/<pid>/fd` entry `socket:[inode]`. An inode owned by more than one process clears the pid and counts `shared_socket_inode`. Unreadable foreign descriptors count `foreign_process_fds_unreadable`. That count is expected for other users and is not a failed collection.

Windows reads `GetExtendedTcpTable` with `TCP_TABLE_OWNER_PID_ALL` and `GetExtendedUdpTable` with `UDP_TABLE_OWNER_PID`, for IPv4 and IPv6. The owner pid in the table is observed. A too-small buffer is `ERROR_INSUFFICIENT_BUFFER` (122) or `ERROR_BUFFER_OVERFLOW` (111); both grow the buffer. Ports are the low 16 bits in network order. IPv4 addresses are little-endian host words.

Direction does not use the port number and does not treat port 443 as encryption.

| State | Direction | Basis |
|---|---|---|
| Listen | listening | observed |
| SYN sent | outbound | observed |
| SYN received | inbound | observed |
| Established, or another TCP state, matched to a local listener including `0.0.0.0` and `::` | inbound or outbound | inferred |
| UDP | unknown | unknown |

`collect` rejects a wait above one second, more than 64 captured packets, or a caller that asks for either. The endpoint list keeps at most 8,192 rows and records `endpoint_sample_truncated` for the rest. Every endpoint from a connection table has payload reason `table_has_no_payload`.

## Encryption and capture gaps

Every snapshot records `driver_image_absent` and `payload_requires_capture`. Connection tables do not contain payload bytes.

Linux may open `AF_PACKET` `SOCK_RAW` for `ETH_P_ALL`. `EPERM` becomes `capture_inaccessible` with the errno. An open socket that returns no payload-bearing frame becomes `capture_no_payload`. A TLS record is content type `0x16`, a version from `0x0301` through `0x0304`, and a length that matches the record. That frame is `PacketBody::Encrypted` with evidence `tls-record-header`. Anything else with a payload is plain. Frames stay in memory. A frame longer than 2,048 bytes is truncated and cannot be persisted as a complete packet. Loopback frames on this path are Ethernet and are duplicated for transmit and receive. A TCP segment with an empty payload, including SYN, is dropped.

Windows may open `SOCK_RAW` with `IPPROTO_IP` and does not set `SIO_RCVALL`. That socket does not see loopback. Its buffers are IP packets, not Ethernet frames. An administrator can open the socket and still record `capture_no_payload` when no payload arrives. Tests accept a classified TLS record, `capture_inaccessible` with an OS code, or `capture_no_payload`. A run that cannot see the bytes records the gap. It does not invent an encrypted body.

## Protocol projection

`flow_event` and `interface_event` build protocol version 1 observations. The provider is `winstrata.net`. Flow events use `net.flow`. Interface events use `net.interface`. The process field stays empty: a pid without a process start time is not a stable process identity, so the pid remains in the evidence string.

A flow whose direction basis is `observed` uses observed attribution. A basis of `inferred` uses inferred attribution. Unknown basis with no pid uses unknown attribution. Flow completeness is partial. The reason is `process_unavailable` when the pid is absent and `table_has_no_payload` when the pid is present. An interface with an unknown operational state is partial with reason `operstate_unknown`. A known operational state is complete and observed.

## Selective persistence

`PacketLedger::memory_only` is the default. `persist` then returns `memory_only` and creates no file, including when a tag is supplied. Untagged bytes are not written and then deleted.

`DiskPolicy::TaggedOnly` writes one named packet. The checks run in this order: unknown id, memory-only policy, missing tag, rejected tag, then empty, truncated or already released bytes. Only the last step opens the path with `create_new`, writes the retained bytes and syncs them. A failed authorized write removes its own partial file. There is no persist-all operation.

A qualifying tag has `reason`, `source` and `confidence`, each 1 to 128 bytes, with no surrounding whitespace and no control characters. `time_unix_ns` is stored with the tag and is not a product risk score. Product tagging remains plan 029. The default memory cap is 256 KiB. When a new packet does not fit, the oldest retained bytes are released. Persisting a released or truncated id returns `unavailable`.

This plan also does not persist untagged connection metadata. That retention choice remains the open owner decision in [Network and data](DATA.md#section-7).

## Boundaries

Production Core does not depend on the kernel crate. The kernel crate does not depend on Core. Core tests may use the kernel as a development dependency. No new crate version is added. Linux uses the already approved `libc` 0.2.191. Windows uses the already approved `windows-sys` 0.61.2 and enables the IpHelper, Ndis and WinSock features those APIs require.

File, registry, process, thread and module collection remain plans 009 and 010. Shared storage is 012. Encryption at rest is 013. Plugin contracts are 014. Topology, discovery, PCAPNG export and the product risk-tag experience remain plan 031. REQ-09.02, REQ-10.02 and REQ-10.03 are not this plan. E2E-04 is not complete.

Commands, counts and the checks that were not run are in the [engineering record](../records/STATUS.md).
