# 网络源、接口与归因

[English](../../architecture/NETWORK-SOURCES.md) · [观测器](../../../kernel/src/net/mod.rs) · [报文账本](../../../core/src/network.rs) · [计划 011](../roadmap/PLAN.md#pr-011)

计划 011 在用户态观测网络接口和 TCP/UDP 端点，归因方向，并把原始报文留在内存。`CatalogHost` 仍然返回 `image_absent`。本计划不加载 `strata-net.sys`，不调用服务控制管理器，也不改变驱动签名、Secure Boot 或 HVCI。观测入口是 `Observer::open` 和 `Observer::collect`，不经过 `Manager::subscribe`。

## 接口

Linux 读取 `getifaddrs` 和 `/sys/class/net`。接口种类来自接口类型、bridge 目录、`IFF_LOOPBACK` 和 `IFF_POINTOPOINT`。名称前缀不是种类。`admin_up` 跟随 `IFF_UP`。`operational` 取 `up`、`down`、`lowerlayerdown`；缺少 `operstate` 时为未知。

Windows 读取 `GetAdaptersAddresses`。环回的接口类型是 24，以太网 6，无线 71，隧道 131，PPP 23，厂商虚拟接口 53。只有 `PhysicalAddressLength` 为 6 时才记录 MAC。Windows 没有单独的管理启用标志，因此仅当 `IfOperStatusUp` 时 `admin_up` 为真。Down 和 lower-layer-down 的运行状态为假。其余状态保持未知。

## 端点与方向

Linux 读取 `/proc/net/tcp`、`tcp6`、`udp` 和 `udp6`。进程号来自 `/proc/<pid>/fd` 中的 `socket:[inode]`。同一个 inode 属于多个进程时清空进程号，并计入 `shared_socket_inode`。读不到的其他用户描述符计入 `foreign_process_fds_unreadable`。这是其他用户的预期结果，不是采集失败。

Windows 对 IPv4 和 IPv6 读取 `GetExtendedTcpTable`（`TCP_TABLE_OWNER_PID_ALL`）和 `GetExtendedUdpTable`（`UDP_TABLE_OWNER_PID`）。表中的所有者进程号按观测记录。缓冲区不足时返回 `ERROR_INSUFFICIENT_BUFFER`（122）或 `ERROR_BUFFER_OVERFLOW`（111），两种都扩大缓冲区后重试。端口取低 16 位的网络序。IPv4 地址是小端主机字。

方向不使用端口号，也不把 443 当成加密。

| 状态 | 方向 | 依据 |
|---|---|---|
| Listen | listening | observed |
| SYN sent | outbound | observed |
| SYN received | inbound | observed |
| Established 或其他 TCP 状态，且能对应到本机监听，包括 `0.0.0.0` 和 `::` | inbound 或 outbound | inferred |
| UDP | unknown | unknown |

`collect` 拒绝超过一秒的等待、超过 64 个采样报文，以及超出这两项上限的调用。端点列表最多保留 8,192 行，其余计入 `endpoint_sample_truncated`。来自连接表的每个端点，其载荷原因都是 `table_has_no_payload`。

## 加密与采样缺口

每次快照都记录 `driver_image_absent` 和 `payload_requires_capture`。连接表不含载荷字节。

Linux 可以打开 `AF_PACKET` `SOCK_RAW`，协议为 `ETH_P_ALL`。`EPERM` 成为带 errno 的 `capture_inaccessible`。套接字已打开但没有带载荷的帧时，记录 `capture_no_payload`。TLS 记录的内容类型是 `0x16`，版本在 `0x0301` 到 `0x0304` 之间，并且长度与记录一致。这样的帧是 `PacketBody::Encrypted`，证据为 `tls-record-header`。其余带载荷的帧是明文。帧只留在内存。超过 2,048 字节的帧会被截断，截断后的帧不能作为完整报文落盘。这条路径上的环回帧是以太网帧，发送和接收各出现一次。空载荷的 TCP 段，包括 SYN，会被丢弃。

Windows 可以打开 `IPPROTO_IP` 的 `SOCK_RAW`，并且不设置 `SIO_RCVALL`。读取前，它绑定到接口列表中的一个非环回 IPv4 地址；没有这样的地址时绑定 `0.0.0.0`。未绑定的读取返回 Winsock 错误 10022，这个错误不能丢掉连接表；该错误以及之后的读取错误都成为带操作系统错误码的 `capture_inaccessible`。该套接字看不到环回。缓冲区是 IP 包，不是以太网帧。管理员可以打开并绑定套接字，但在没有载荷到达时仍然记录 `capture_no_payload`。测试接受已分类的 TLS 记录、带操作系统错误码的 `capture_inaccessible`，或 `capture_no_payload`。看不到字节时记录缺口，不编造加密报文。

## 协议投影

`flow_event` 和 `interface_event` 生成协议版本 1 的观测记录。提供者为 `winstrata.net`。流事件类型是 `net.flow`，接口事件类型是 `net.interface`。进程字段保持为空：没有进程启动时间的进程号不是稳定的进程身份，进程号留在证据字符串里。

方向依据为 `observed` 时使用观测归因。依据为 `inferred` 时使用推断归因。依据未知且没有进程号时使用未知归因。流的完整度是部分完整。没有进程号时原因是 `process_unavailable`，有进程号时原因是 `table_has_no_payload`。运行状态未知的接口是部分完整，原因是 `operstate_unknown`。运行状态已知时为完整且已观测。

## 选择性留存

`PacketLedger::memory_only` 是默认策略。此时 `persist` 返回 `memory_only`，并且不创建文件，即使调用方提供了标记。未标记字节不会先写入再删除。

`DiskPolicy::TaggedOnly` 只写入被点名的一个报文。检查顺序是：未知编号、仅内存策略、缺少标记、标记不合格，然后是空字节、已截断或已被释放。只有最后一步才用 `create_new` 打开路径、写入保留字节并同步。已授权写入如果失败，会删除自己的半截文件。没有全部落盘操作。

合格标记的 `reason`、`source` 和 `confidence` 各为 1 到 128 字节，没有首尾空白，也没有控制字符。`time_unix_ns` 与标记一起保存，它不是产品风险分。产品标记是计划 029，见[标记插件](TAG.md)。拓扑、发现和 PCAPNG 导出仍属于计划 031。默认内存上限是 256 KiB。新报文放不下时释放最旧的已保留字节。对已释放或已截断的编号执行 `persist` 会返回 `unavailable`。

本计划也不把未标记的连接元数据写入磁盘。这项留存选择仍是[网络与数据](DATA.md#section-7)中的待决事项。

## 边界

Core 依赖 kernel crate，因为计划 009 的文件和注册表采集在那里运行。kernel crate 不依赖 Core。没有新增 crate 版本。Linux 使用已经批准的 `libc` 0.2.191。Windows 使用已经批准的 `windows-sys` 0.61.2，并启用这些 API 所需的 IpHelper、Ndis 和 WinSock 特性。

文件、注册表、进程、线程和模块采集仍属于计划 009 和 010。共享存储是 012。静态加密由计划 013 实现。插件契约是 014。拓扑、发现、PCAPNG 导出和产品风险标记体验仍属于计划 031。REQ-09.02、REQ-10.02 和 REQ-10.03 不属于本计划。E2E-04 未完成。

命令、计数和尚未执行的检查见[工程记录](../records/STATUS.md)。
