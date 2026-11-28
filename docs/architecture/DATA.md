# Network and data policy

[简体中文](../zh-CN/architecture/DATA.md) · [Index](../INDEX.md)

<a id="section-7"></a>

## Network visibility and packet retention

Observe available physical and virtual interfaces, VPN, TUN, loopback, IPv4/IPv6, TCP, UDP, ICMP, DNS, HTTP, TLS and QUIC where visible. Show incoming and outgoing traffic, ports, process/software/service attribution, system proxy, firewall state, connections, and topology. Scope authorized local-network discovery by range, frequency, and budgets. Show devices, MAC/vendor, ports, services, routes, and presence changes. Network activity must preserve traffic integrity and remain within authorization. Distinguish observed from inferred links and show gaps in encrypted or inaccessible traffic.

1. Parse and attribute live packets in bounded memory.
2. Record risk tags for network and other objects from the user, scripts, plugins and authorized local agents. Record reason, source, time, and confidence.
3. Only a qualifying risk tag permits persistence of a raw packet and specifically controlled context. Query and export PCAP or PCAPNG. Untagged raw packets never reach disk, including temporary files. Release them when their memory retention expires.
4. Let users configure memory cap and retention. Report too-late tags and discarded or unavailable data. Disabling tags cannot switch on full packet persistence. A missing tag carries no safety conclusion.
5. Preserve traceable filter, tag and resource decisions. Support live statistics and distinguish intentional filtering from collection loss.

Pending owner decision covers retention of lightweight untagged connection metadata such as IP, domain, process, time, or size. Optional metadata and aggregate history is a candidate under the appropriate log quota. The raw packet rule applies regardless. Plan 011 implements the user-mode observer and the selective raw-packet gate in [Network sources](NETWORK-SOURCES.md). It does not persist untagged connection metadata and does not export PCAPNG. Plan 029 records the tag and uses that ledger. See [Tag plugin](TAG.md). Topology, discovery, and PCAPNG export remain plan 031.

<a id="section-8"></a>

## Hybrid storage and protection

Use structured storage for config, software, objects, and relations, append-only segments for high-rate events, compressed time-series for metrics, PCAPNG or PCAP segments for risk-tagged packets, and full/incremental state for snapshots. Share indexes and retain underlying records once. Core owns I/O, query, index, permissions, encryption, backup, retention, migration, state consistency, and diagnostic handling. Plugins declare schemas. Cache derived values only when justified. Handle corrupt data and stale references.

Default to `D:\WinStrata\`. If D is missing, unwritable or full, require a valid user-selected directory and suspend operations requiring persistence until available. Warn on storage loss and avoid automatic volume switching. Verify migration and retain the source until confirmed. Separate runtime data from source and build files. Under the data root, candidates include databases, events, metrics, captures, snapshots, indexes, plugins, runtimes, config, logs, backups and cache. Resolve all locations centrally. Keep minimal OS registration separate and show path, capacity, filesystem, permission, and migration limitations in the GUI.

Plan 012 places those six `data` directories in each user's private directory. The implemented layout, reads and writes, segments, index, capacity and corruption rollback are specified in [Core storage](CORE-STORAGE.md). No SQLite crate was added. Structured records use versioned segments. Plan 013 adds optional encryption, backup, restore and encrypted migration there. Plan 030 performs confirmed cleanup in [Storage plugin](STORAGE.md). [OI-024](../records/OPEN_ITEMS.md#oi-024) stays open: units, saved-log accounting beyond the logs directory, the default cleanup selection, and expiration authorization are not confirmed. The ladder uses the decimal candidate and does not delete on expiration.

### Retention and capacity

Default retention is indefinite subject to the raw capture rule. Allow independent retention settings. Expiration authorization semantics, deletion defaults, log accounting boundaries and unit convention require owner approval. Decimal GB/MB, including applicability to structured events, metrics, indexes or backups, is a candidate. Do not use classification to circumvent thresholds. The user must explicitly confirm capacity cleanup independently of configured retention.

| Data | First warning | Subsequent increase after decline | Configuration |
|---|---:|---:|---|
| Persisted raw capture | 1 GB | 1 GB | GUI and script |
| Saved logs | 2 GB | 500 MB | GUI and script |

Count captures and logs separately. A warning is an acknowledgement and cleanup decision. Declining continues retention and schedules the next threshold. Show current usage, category, proposed policy and scope, estimated space and next warning. No response grants no deletion authority. Persist warning state across restarts and recalculate on cleanup. Verify changed thresholds, multiple thresholds crossed in one batch, residual overage and tray behavior.

StrataStorage supports time, risk, expiration, capacity, type, selection, protected tags and script-defined scopes. Show actual scope before execution. Scripts and MCP may configure, analyze and propose policies, with user confirmation before deletion. Core retains basic alerts if the plugin is disabled. Disk exhaustion, write limits, or budget pressure must warn and mark unpersisted ranges. Restrict cleanup to WinStrata-managed data. Synchronize indexes, references and audit data and favor segment deletion to reduce write amplification and repeated alerts.

### Filtering and device protection

Core suppresses feedback from ordinary internal writes, logs and IPC, and observes relevant modifications to WinStrata files, settings and databases by external processes. StrataHealth and Core errors cover internal diagnostics. Keep optional debug self-observation bounded.

StrataFilter can include or exclude process, app, path, event, protocol, IP, port and interface, with deduplication, aggregation, non-critical sampling, batch intervals, buffers, compression and archival policy. Show counts, avoided writes and overhead. Run script filters in batches through StrataScript. On errors, log and use safe basic filtering. Preserve safety, data integrity and selective retention across plugin states. Plan 028 implements that behavior in [Filter plugin](FILTER.md). Baseline suppression still applies when the filter process is not running. E2E-04, E2E-05, E2E-07, and E2E-08 stay open.

Use incremental indexes, event-driven collection, ordered batches, bounded in-memory staging and compression. Reduce rescans, repeated reads, full-disk hashes, sync writes and amplification while preserving crash consistency. Core enforces baseline protection through the [Core runtime](CORE-RUNTIME.md). StrataBudget governs overall and per-plugin CPU, memory, disk, network, storage, queue, concurrency and GUI refresh targets. Plan 026 records pause, resume, and a loss marker for this process's CPU and resident set and for the queues attached to the plugin host, without lowering precision. Disk I/O, network, storage, and GUI refresh budgets remain [OI-051](../records/OPEN_ITEMS.md#oi-051). The behavior is in [Resource budgets](BUDGET.md). StrataDisk reads SMART and counters on demand or at low frequency. Distinguish logical program writes from device-supported actual write counts and mark unsupported wear values. StrataHealth shows live activity, loss, queues, service, driver and storage health. Plan 025 reads faults, queues, latency and loss from the bus and scheduler attached to the plugin host, and from plugin fault state, and writes that reading to the plugin's private report. Service, driver, storage and GUI readings remain later plans, recorded in [Health diagnostics](HEALTH.md). Core retains essential errors when the plugin is disabled.

### Encryption and portability

Offer optional encryption of databases, captures, snapshots, logs and backups with independent password, auto-lock, rotation, password change, protected keys and encrypted migration. Plan 013 seals the hybrid payloads and the vault with the operating system's AES-256-GCM and PBKDF2-HMAC-SHA256. The portable vault does not use DPAPI. Validate key derivation and storage and use established cryptography. Keep passwords, keys and recovery material local. Locking the store blocks reads while commits continue. The GUI lock display remains plan 042. Plugins use authorized data access without holding Core keys. Security relies on the OS trust boundary.

Support full and incremental backup, restore, plugin and time-range export, integrity checks, device migration and history merging with device identity. Plan 013 writes a single-file full or incremental package, checks each file CRC, and restores it without requiring the destination device id to match. Plugin export, time-range export and plaintext export remain plans 038 and 042. Candidates include JSON/JSONL, CSV, HTML, PCAP/PCAPNG, SQLite backups and a project package. Validate import format, permissions, version, source, malicious inputs, paths and references. Confirm plaintext export of encrypted material. Process data locally.
