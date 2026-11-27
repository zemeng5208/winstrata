# Product and official plugins

[简体中文](../zh-CN/architecture/PRODUCT.md) · [Index](../INDEX.md)

<a id="section-2"></a>

## Product scope

WinStrata observes Windows system state, traces changes, analyzes software lifecycles and network activity, and supports local extensions. It records system states, metadata, provenance, and real-time events. It ships as a development build with source, SDKs, debug and development tools. The planned release is governed by the [acceptance plan](../roadmap/ACCEPTANCE.md).

| Area | Requirement |
|---|---|
| Start and stop | The user starts monitoring. Optional start at boot defaults off. Collection stops with monitoring. |
| Users | Observe the current Windows user and relevant public system activity. Protect other users' private data. |
| Platforms | Windows 10 and 11 x64 runtime. Linux and Windows development. Server, ARM64, and x86 are outside the first release. |
| Scope | The user selects disks, partitions, directories, and devices. New devices are excluded until selected. |
| Limits | Monitoring, recording, analysis, and querying are read-only. Only the [restricted StrataRescue channel](EXTENSIONS.md#section-6) may execute explicitly authorized system changes. |
| Network | Allow update and market-index requests. Operate without proprietary cloud dependencies. Keep telemetry, logs, files, hashes, and all observed data local. |
| Interfaces | A built-in desktop GUI and explicitly authorized local MCP, with no general end-user CLI. The official GUI cannot be uninstalled and is the recovery interface. A frontend plugin may replace the active GUI under [AG-64](../../AGENTS.md#ag-64). That replacement is not one of the 25 plugins below. |
| AI | External platforms own agent operation, inference, scheduling, and API keys. |
| Versioning | System state, metadata, and provenance are tracked. File content versions and diffs remain the responsibility of version control. |
| Languages | Complete Simplified Chinese and English, runtime switching, and English fallback. |
| Delivery | Development build only, with Windows security protections enabled. |

User confirmation is required to publish plugins or scripts. Ordinary network requests carry no observed data. External platforms govern use of the information their agents read. Identify gaps from downtime, inaccessible kernel activity, unselected scope, user isolation, encryption, inaccessible or unauthorized topology, and unsupported hardware. Label reconstructed history and inference.

<a id="section-4"></a>

## Official native Rust plugins

Ship all 25 built-ins. Users choose which to enable on first launch. Disablement releases exclusive resources. Built-ins remain installed and support individual updates and rollback. Compatible plugins may replace capabilities within their granted permissions. Rescue authority is exclusive. Review working names in PR 000 and obtain approval before changing the set.

| ID | Plugin | Required behavior |
|---|---|---|
| PL-01 | StrataPulse | CPU, GPU, memory, VRAM, disks, load, process use, frequency, temperature, power, battery, sensors, trends and peaks. Label unsupported readings. |
| PL-02 | StrataNet | Visible-interface capture, inbound and outbound traffic, protocol parsing, process attribution, topology, authorized discovery, proxy/VPN/TUN/loopback, IP/MAC/DNS/routes/ARP, connection history. |
| PL-03 | StrataTrace | System and file changes, create/modify/delete/move, path, source, size, permissions, ownership, hash, settings and causality. Preserve metadata without content versions. |
| PL-04 | StrataApps | Install, update, run, and uninstall lifecycle, automatic and manual sessions, historical software, remnants, attribution, and distributed disk usage. |
| PL-05 | StrataRegistry | Current-user and relevant system key and value changes, before and after values, attribution, and queries. |
| PL-06 | StrataEnv | Current-user and relevant system environment and PATH, before and after values and attribution. |
| PL-07 | StrataProcess | Process trees, PID, path, command line, threads, modules and DLLs, lifecycle, and provenance. |
| PL-08 | StrataServices | Service and driver creation, deletion, load, unload, startup type, status, and configuration changes. |
| PL-09 | StrataStartup | Startup folders, registry entries, scheduled tasks, persistence locations, state, and changes. |
| PL-10 | StrataSecurity | Read-only Defender, firewall, UAC, ACLs, elevation, Secure Boot, TPM, BitLocker, certificates and roots, group and security policy, and observable injection and DLL activity. |
| PL-11 | StrataSnapshot | Manual full and incremental snapshots, selected scope, differences, queries, deletion, import and export. Automatic by user setup only. No bundled automation or rollback. |
| PL-12 | StrataRules | Visual rules, filters, correlations, anomaly detection, thresholds, local notifications, and import/export. Triggers record, analyze, or notify. |
| PL-13 | StrataHealth | Core, plugin, driver, queue, IPC, storage and GUI diagnostics, including resources, latency, throughput, loss, errors, permissions and version, with local reports. |
| PL-14 | StrataTimeline | Multi-track timeline, zoom, filters, tags, sync, playback, evidence jumps, and range export. Playback has no system side effects. |
| PL-15 | StrataBudget | Global and plugin CPU, memory, disk I/O, network, storage, queue, task, and GUI budgets. Throttle, defer, or pause and mark gaps. |
| PL-16 | StrataMCP | Authorized local-agent queries, plugin development, management and debugging, script editing, and market workflows with dynamic discovery and auditing. |
| PL-17 | StrataRescue | Default-off temporary restricted local-agent channel without its own detection or repair policy. |
| PL-18 | StrataFilter | GUI and script filters for events, traffic and internal activity. Deduplication, aggregation, sampling, write and compression policy, and statistics. |
| PL-19 | StrataDisk | SSD/HDD/NVMe, SMART, health, lifespan, temperature, accumulated I/O, process attribution, WinStrata logical writes and warnings. |
| PL-20 | StrataScript | Editing, debugging, execution, permissions, dependencies, versions, logs, event/schedule/condition triggers, import/export, and MCP. No presets. |
| PL-21 | StrataDependency | Software, DLL, runtime, shared library, service and driver graphs, missing dependencies, conflicts, ownership and remnants. |
| PL-22 | StrataTrust | Signatures, publishers, chains, hashes, integrity, origin and update differences. Keep files and hashes local. Describe unsigned status objectively. |
| PL-23 | StrataUpdate | Read-only Windows Update, KBs, drivers, optional features, errors, pending restart, related changes and cache or residual size. |
| PL-24 | StrataTag | Manual, script, plugin and local-agent risk tags with source, evidence, time and confidence. Controls selective raw packet retention. |
| PL-25 | StrataStorage | Capacity warnings, GUI/script thresholds, policies, estimates, confirmation and cleanup of WinStrata-managed data. |

### Software lifecycle and shared sources

Baseline selected software, files, registry, environment, services, drivers, tasks, and possible remnants. Keep manual snapshots independent. Label reconstructed history and refresh using real events and incremental scans. Detect MSI, MSIX, EXE, portable and scripted installs, updates and uninstalls. Attribute using process trees, indirect service activity, shared dependencies and observed changes. Manual sessions target an installer, script, process, or portable app and compare initial and final state. Timing overlap alone is insufficient.

Profiles show confirmed paths, registry before/after, environment, services, drivers, startup, tasks, shell integration including COM, context menus and file associations, network, resources, signatures, dependencies, and timeline. Mark system and shared components and ambiguous ownership. Give remnant locations, evidence, and confidence. Keep general cleaning and forced uninstall outside first-release scope.

Show program, config, cache, logs, user data, updates, downloads, remnants, and shared disk usage, with details and trends. Separate logical and actual size and handle hard links and sparse files. Hash on demand and reuse indexes and incremental scans. Share sources across StrataTrace, StrataRegistry, StrataSecurity, StrataTrust, and StrataUpdate. Label unavailable values and provide related authorized MCP queries.
