# Requirements traceability

[简体中文](../zh-CN/roadmap/REQUIREMENTS.md) · [Index](../INDEX.md)

The linked authoritative statements govern scope and criteria. Runtime evidence is pending for planned work. The baseline and verified development-toolchain evidence are recorded separately. Update evidence and state for affected rows in each PR.

## Product and engineering requirements

| ID | Requirement | Authoritative source | Plan PR | Verify | Status or evidence |
|---|---|---|---|---|---|
| REQ-01.01 | Windows system observation and change tracking | [PRODUCT](../architecture/PRODUCT.md#section-2) | [000](PLAN.md#pr-000) | [16](ACCEPTANCE.md#e2e-16) | Planned |
| REQ-01.02 | Public monorepo, main and Apache-2.0 baseline | [PRODUCT](../architecture/PRODUCT.md#section-2) | [000](PLAN.md#pr-000) | [16](ACCEPTANCE.md#e2e-16) | [Baseline verified](../records/STATUS.md#baseline) |
| REQ-01.03 | Deliver source, SDK, debug and development build | [ACCEPTANCE](../roadmap/ACCEPTANCE.md#section-20) | [050](PLAN.md#pr-050) | [16](ACCEPTANCE.md#e2e-16) | Planned |
| REQ-02.01 | Actual Windows 10 and 11 x64 operation | [SYSTEM](../architecture/SYSTEM.md#section-3) | [004](PLAN.md#pr-004), [049](PLAN.md#pr-049) | [14](ACCEPTANCE.md#e2e-14), [16](ACCEPTANCE.md#e2e-16) | Planned |
| REQ-02.02 | Build complete Windows x64 artifacts on native Linux | [SYSTEM](../architecture/SYSTEM.md#section-3) | [003](PLAN.md#pr-003), [050](PLAN.md#pr-050) | [16](ACCEPTANCE.md#e2e-16) | Linux console and kernel linking verified. Full artifact pipeline pending. |
| REQ-02.03 | Keep both build capabilities and verify the available environment | [DEPENDENCIES](../standards/DEPENDENCIES.md#ag-35) | [001](PLAN.md#pr-001), [050](PLAN.md#pr-050) | [16](ACCEPTANCE.md#e2e-16) | Linux tooling verified. Cross-platform build acceptance pending. |
| REQ-03.01 | Ordinary GUI and Core, controlled service and kernel privilege split | [SYSTEM](../architecture/SYSTEM.md#section-3) | [008](PLAN.md#pr-008) | [14](ACCEPTANCE.md#e2e-14), [15](ACCEPTANCE.md#e2e-15) | Planned |
| REQ-03.02 | Version and authenticate IPC with validation and audit | [SYSTEM](../architecture/SYSTEM.md#section-3) | [005](PLAN.md#pr-005), [008](PLAN.md#pr-008) | [14](ACCEPTANCE.md#e2e-14), [15](ACCEPTANCE.md#e2e-15) | Planned |
| REQ-03.03 | Supported mixed collection and independent on-demand driver lifetime | [SYSTEM](../architecture/SYSTEM.md#section-3) | [008–011](PLAN.md#pr-008) | [02](ACCEPTANCE.md#e2e-02), [04](ACCEPTANCE.md#e2e-04), [14](ACCEPTANCE.md#e2e-14), [15](ACCEPTANCE.md#e2e-15) | Planned |
| REQ-03.04 | Keep signing, Secure Boot, and HVCI in the supported runtime | [SECURITY](../standards/SECURITY.md#ag-47) | [008](PLAN.md#pr-008) | [14](ACCEPTANCE.md#e2e-14), [15](ACCEPTANCE.md#e2e-15) | Planned |
| REQ-04.01 | Scope to current user, relevant public system events and chosen storage | [PRODUCT](../architecture/PRODUCT.md#section-2) | [006](PLAN.md#pr-006), [017](PLAN.md#pr-017) | [01](ACCEPTANCE.md#e2e-01), [02](ACCEPTANCE.md#e2e-02) | Planned |
| REQ-04.02 | Onboard with opt-in plugins, manual start and boot disabled by default | [FRONTEND](../architecture/FRONTEND.md#section-9) | [017](PLAN.md#pr-017) | [01](ACCEPTANCE.md#e2e-01), [11](ACCEPTANCE.md#e2e-11) | Planned |
| REQ-04.03 | Build selected baseline and label reconstructed history | [PRODUCT](../architecture/PRODUCT.md#section-4) | [017](PLAN.md#pr-017), [021](PLAN.md#pr-021) | [01](ACCEPTANCE.md#e2e-01), [02](ACCEPTANCE.md#e2e-02) | Planned |
| REQ-04.04 | Live/history switching, pause/resume, gaps and safe shutdown | [FRONTEND](../architecture/FRONTEND.md#section-9) | [017](PLAN.md#pr-017) | [11](ACCEPTANCE.md#e2e-11) | Planned |
| REQ-05.01 | Stable event identity/version, ordered timestamps, source and completeness | [SYSTEM](../architecture/SYSTEM.md#section-3) | [005](PLAN.md#pr-005) | [02](ACCEPTANCE.md#e2e-02), [15](ACCEPTANCE.md#e2e-15) | Planned |
| REQ-05.02 | Bound batch, queue, backpressure, throughput and loss accounting | [SYSTEM](../architecture/SYSTEM.md#section-3) | [007](PLAN.md#pr-007) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | Planned |
| REQ-05.03 | Central priorities, timeouts, cancellation and resource protections | [ARCHITECTURE](../standards/ARCHITECTURE.md#ag-45) | [007](PLAN.md#pr-007), [025](PLAN.md#pr-025), [026](PLAN.md#pr-026) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | Planned |
| REQ-06.01 | Observe file operations, ownership and permissions without content versions | [PRODUCT](../architecture/PRODUCT.md#section-4) | [009](PLAN.md#pr-009), [021](PLAN.md#pr-021) | [02](ACCEPTANCE.md#e2e-02) | Planned |
| REQ-06.02 | Observe processes, threads, modules, provenance and PID lifecycle | [PRODUCT](../architecture/PRODUCT.md#section-4) | [010](PLAN.md#pr-010), [018](PLAN.md#pr-018) | [02](ACCEPTANCE.md#e2e-02), [03](ACCEPTANCE.md#e2e-03) | Planned |
| REQ-06.03 | Attribute registry and environment changes with before and after values | [PRODUCT](../architecture/PRODUCT.md#section-4) | [019](PLAN.md#pr-019), [020](PLAN.md#pr-020) | [02](ACCEPTANCE.md#e2e-02) | Planned |
| REQ-06.04 | Observe service, driver, startup and task state changes | [PRODUCT](../architecture/PRODUCT.md#section-4) | [022](PLAN.md#pr-022), [023](PLAN.md#pr-023) | [03](ACCEPTANCE.md#e2e-03), [14](ACCEPTANCE.md#e2e-14) | Planned |
| REQ-07.01 | Identify automatic and manual software lifecycle sessions | [PRODUCT](../architecture/PRODUCT.md#section-4) | [035](PLAN.md#pr-035) | [03](ACCEPTANCE.md#e2e-03) | Planned |
| REQ-07.02 | Attribute software profiles, remnant evidence and disk usage | [PRODUCT](../architecture/PRODUCT.md#section-4) | [035](PLAN.md#pr-035) | [03](ACCEPTANCE.md#e2e-03), [09](ACCEPTANCE.md#e2e-09) | Planned |
| REQ-07.03 | Trace actual dependency, signature, integrity and origin evidence | [PRODUCT](../architecture/PRODUCT.md#section-4) | [033](PLAN.md#pr-033), [034](PLAN.md#pr-034) | [03](ACCEPTANCE.md#e2e-03), [09](ACCEPTANCE.md#e2e-09) | Planned |
| REQ-08.01 | Show supported CPU/GPU/memory, power and sensor data with history | [PRODUCT](../architecture/PRODUCT.md#section-4) | [024](PLAN.md#pr-024) | [08](ACCEPTANCE.md#e2e-08) | Planned |
| REQ-08.02 | Diagnose and regulate resources with traceable degradation | [DATA](../architecture/DATA.md#section-8) | [025](PLAN.md#pr-025), [026](PLAN.md#pr-026) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | Planned |
| REQ-08.03 | Measure drive health, logical and physical writes and I/O | [DATA](../architecture/DATA.md#section-8) | [032](PLAN.md#pr-032), [048](PLAN.md#pr-048) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | Planned |
| REQ-09.01 | Observe available interfaces and attribute inbound and outbound connections | [DATA](../architecture/DATA.md#section-7) | [011](PLAN.md#pr-011), [031](PLAN.md#pr-031) | [04](ACCEPTANCE.md#e2e-04) | Planned |
| REQ-09.02 | Authorized discovery, topology and explicit visibility limits | [DATA](../architecture/DATA.md#section-7) | [031](PLAN.md#pr-031) | [04](ACCEPTANCE.md#e2e-04) | Planned |
| REQ-10.01 | Record risk source, evidence, time and confidence | [DATA](../architecture/DATA.md#section-7) | [029](PLAN.md#pr-029) | [04](ACCEPTANCE.md#e2e-04) | Planned |
| REQ-10.02 | Keep untagged raw packets off disk from capture to disposal | [DATA](../architecture/DATA.md#section-7) | [031](PLAN.md#pr-031) | [04](ACCEPTANCE.md#e2e-04), [05](ACCEPTANCE.md#e2e-05) | Planned |
| REQ-10.03 | Use bounded configurable packet buffer and risk capture export | [DATA](../architecture/DATA.md#section-7) | [031](PLAN.md#pr-031) | [04](ACCEPTANCE.md#e2e-04) | Planned |
| REQ-11.01 | Default to D drive with manual fallback and safe migration | [DATA](../architecture/DATA.md#section-8) | [006](PLAN.md#pr-006) | [01](ACCEPTANCE.md#e2e-01), [10](ACCEPTANCE.md#e2e-10) | Planned |
| REQ-11.02 | Support structured, segmented, time-series, capture and snapshot storage | [DATA](../architecture/DATA.md#section-8) | [012](PLAN.md#pr-012) | [05](ACCEPTANCE.md#e2e-05), [10](ACCEPTANCE.md#e2e-10) | Planned |
| REQ-11.03 | Use shared identity, one source record and coordinated indexing | [ARCHITECTURE](../standards/ARCHITECTURE.md#ag-44) | [012](PLAN.md#pr-012), [040](PLAN.md#pr-040) | [09](ACCEPTANCE.md#e2e-09) | Planned |
| REQ-12.01 | Keep eligible data indefinitely by default with configurable retention | [DATA](../architecture/DATA.md#section-8) | [012](PLAN.md#pr-012), [030](PLAN.md#pr-030) | [05](ACCEPTANCE.md#e2e-05) | Planned |
| REQ-12.02 | Persist capture 1 GB and log 2 GB warning state, repeat at 1 GB and 500 MB | [DATA](../architecture/DATA.md#section-8) | [030](PLAN.md#pr-030) | [05](ACCEPTANCE.md#e2e-05) | Planned |
| REQ-12.03 | Require user confirmation for cleanup and synchronize indexes after deletion | [DATA](../architecture/DATA.md#section-8) | [030](PLAN.md#pr-030) | [05](ACCEPTANCE.md#e2e-05) | Planned |
| REQ-12.04 | Retain core alerts, mark unavailable storage and exclude external data from cleanup | [DATA](../architecture/DATA.md#section-8) | [030](PLAN.md#pr-030) | [05](ACCEPTANCE.md#e2e-05), [15](ACCEPTANCE.md#e2e-15) | Planned |
| REQ-13.01 | Prevent self-recursion but preserve relevant external changes | [DATA](../architecture/DATA.md#section-8) | [028](PLAN.md#pr-028) | [05](ACCEPTANCE.md#e2e-05), [08](ACCEPTANCE.md#e2e-08) | Planned |
| REQ-13.02 | Configure GUI and batch script filters with safe fallback | [DATA](../architecture/DATA.md#section-8) | [027](PLAN.md#pr-027), [028](PLAN.md#pr-028) | [04](ACCEPTANCE.md#e2e-04), [07](ACCEPTANCE.md#e2e-07) | Planned |
| REQ-13.03 | Reduce wasteful I/O and write amplification with measured integrity | [DATA](../architecture/DATA.md#section-8) | [032](PLAN.md#pr-032), [048](PLAN.md#pr-048) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | Planned |
| REQ-14.01 | Keep optional local encryption and locking with ongoing monitoring | [DATA](../architecture/DATA.md#section-8) | [013](PLAN.md#pr-013), [042](PLAN.md#pr-042) | [10](ACCEPTANCE.md#e2e-10) | Planned |
| REQ-14.02 | Verify backup, restore, integrity, migration and device identity | [DATA](../architecture/DATA.md#section-8) | [013](PLAN.md#pr-013), [038](PLAN.md#pr-038) | [10](ACCEPTANCE.md#e2e-10) | Planned |
| REQ-14.03 | Authorize and validate import and exports including plaintext conversion | [DATA](../architecture/DATA.md#section-8) | [038](PLAN.md#pr-038), [042](PLAN.md#pr-042) | [09](ACCEPTANCE.md#e2e-09), [10](ACCEPTANCE.md#e2e-10) | Planned |
| REQ-15.01 | Provide manual scoped full/incremental snapshots and difference views | [PRODUCT](../architecture/PRODUCT.md#section-4) | [038](PLAN.md#pr-038) | [07](ACCEPTANCE.md#e2e-07) | Planned |
| REQ-15.02 | Disable automatic snapshots by default and allow user-defined automation | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [027](PLAN.md#pr-027), [038](PLAN.md#pr-038) | [07](ACCEPTANCE.md#e2e-07) | Planned |
| REQ-16.01 | Provide unified Python/TS/JS workbench, debugging, triggers and history | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [027](PLAN.md#pr-027) | [07](ACCEPTANCE.md#e2e-07), [12](ACCEPTANCE.md#e2e-12) | Planned |
| REQ-16.02 | Run visual rules, thresholds and notifications read-only | [PRODUCT](../architecture/PRODUCT.md#section-4) | [039](PLAN.md#pr-039) | [07](ACCEPTANCE.md#e2e-07) | Planned |
| REQ-16.03 | Enforce script permissions, dependencies and absence of bundled presets | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [015](PLAN.md#pr-015), [027](PLAN.md#pr-027) | [07](ACCEPTANCE.md#e2e-07), [12](ACCEPTANCE.md#e2e-12) | Planned |
| REQ-17.01 | Report Windows Update, KBs, drivers, features, reboot and errors read-only | [PRODUCT](../architecture/PRODUCT.md#section-4) | [036](PLAN.md#pr-036) | [14](ACCEPTANCE.md#e2e-14) | Planned |
| REQ-17.02 | Report observable Windows security and certificate state read-only | [PRODUCT](../architecture/PRODUCT.md#section-4) | [037](PLAN.md#pr-037) | [14](ACCEPTANCE.md#e2e-14) | Planned |
| REQ-18.01 | Search and correlate authorized sources and trace object evidence | [FRONTEND](../architecture/FRONTEND.md#section-9) | [040](PLAN.md#pr-040) | [09](ACCEPTANCE.md#e2e-09) | Planned |
| REQ-18.02 | Provide multi-track timeline, analysis-only playback and range export | [PRODUCT](../architecture/PRODUCT.md#section-4) | [041](PLAN.md#pr-041) | [09](ACCEPTANCE.md#e2e-09) | Planned |
| REQ-19.01 | Verify three required sources and adapt shadcn-admin layout within approved bounds | [FRONTEND](../architecture/FRONTEND.md#section-9) | [004](PLAN.md#pr-004), [016](PLAN.md#pr-016) | [11](ACCEPTANCE.md#e2e-11) | Planned |
| REQ-19.02 | Ship three shared themes with runtime switching and persistence | [FRONTEND](../architecture/FRONTEND.md#section-9) | [016](PLAN.md#pr-016) | [11](ACCEPTANCE.md#e2e-11) | Planned |
| REQ-19.03 | Provide two complete locales and readable Windows WebView2 interaction | [FRONTEND](../architecture/FRONTEND.md#section-9) | [016](PLAN.md#pr-016), [042](PLAN.md#pr-042) | [11](ACCEPTANCE.md#e2e-11) | Planned |
| REQ-19.04 | Persist a single configurable home layout and host dynamic plugin UI | [FRONTEND](../architecture/FRONTEND.md#section-9) | [042](PLAN.md#pr-042) | [11](ACCEPTANCE.md#e2e-11) | Planned |
| REQ-19.05 | Offer tray continuation or full exit and true GUI states | [FRONTEND](../architecture/FRONTEND.md#section-9) | [016](PLAN.md#pr-016), [017](PLAN.md#pr-017) | [01](ACCEPTANCE.md#e2e-01), [11](ACCEPTANCE.md#e2e-11) | Planned |
| REQ-20.01 | Bundle exactly 25 plugins, user opt-in, built-ins remain installed | [PRODUCT](../architecture/PRODUCT.md#section-4) | [014](PLAN.md#pr-014), [017](PLAN.md#pr-017) | [06](ACCEPTANCE.md#e2e-06), [11](ACCEPTANCE.md#e2e-11) | Planned |
| REQ-20.02 | Manage independent lifecycle, release and compatible replacement | [ARCHITECTURE](../standards/ARCHITECTURE.md#ag-43) | [014](PLAN.md#pr-014) | [06](ACCEPTANCE.md#e2e-06) | Planned |
| REQ-20.03 | Verify dynamic upgrades, dependency health and independent fault isolation | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [014](PLAN.md#pr-014), [044](PLAN.md#pr-044) | [06](ACCEPTANCE.md#e2e-06), [15](ACCEPTANCE.md#e2e-15) | Planned |
| REQ-21.01 | Document capability declarations and least-privilege permission lifecycle | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [014](PLAN.md#pr-014), [015](PLAN.md#pr-015) | [06](ACCEPTANCE.md#e2e-06), [07](ACCEPTANCE.md#e2e-07) | Planned |
| REQ-21.02 | Isolate on-demand Python and TS/JS runtime and dependency environments | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [004](PLAN.md#pr-004), [015](PLAN.md#pr-015) | [06](ACCEPTANCE.md#e2e-06), [07](ACCEPTANCE.md#e2e-07), [12](ACCEPTANCE.md#e2e-12) | Planned |
| REQ-21.03 | Provide supported-language SDKs and verify OS-level containment | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [015](PLAN.md#pr-015) | [06](ACCEPTANCE.md#e2e-06), [12](ACCEPTANCE.md#e2e-12) | Planned |
| REQ-22.01 | Provide one local-first market with official, community, installed and update views | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [043](PLAN.md#pr-043) | [06](ACCEPTANCE.md#e2e-06), [12](ACCEPTANCE.md#e2e-12) | Planned |
| REQ-22.02 | Install composite packages and manage sources, warnings and verification | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [043](PLAN.md#pr-043) | [06](ACCEPTANCE.md#e2e-06) | Planned |
| REQ-22.03 | Support free submissions with automatic checks and explicit publication consent | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [043](PLAN.md#pr-043), [046](PLAN.md#pr-046) | [06](ACCEPTANCE.md#e2e-06), [12](ACCEPTANCE.md#e2e-12) | Planned |
| REQ-23.01 | Independently version core components and extensions with pins | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [014](PLAN.md#pr-014), [044](PLAN.md#pr-044) | [06](ACCEPTANCE.md#e2e-06), [15](ACCEPTANCE.md#e2e-15) | Planned |
| REQ-23.02 | Validate signed updates and roll back without losing configuration or history | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [044](PLAN.md#pr-044) | [06](ACCEPTANCE.md#e2e-06), [15](ACCEPTANCE.md#e2e-15) | Planned |
| REQ-24.01 | Authenticate local MCP identities and dynamically revoke exposed capabilities | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [045](PLAN.md#pr-045) | [12](ACCEPTANCE.md#e2e-12) | Planned |
| REQ-24.02 | Expose authorized live and historical queries under budgets and audit | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [045](PLAN.md#pr-045) | [12](ACCEPTANCE.md#e2e-12) | Planned |
| REQ-24.03 | Develop, validate, install and publish plugins and scripts through local MCP | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [046](PLAN.md#pr-046) | [12](ACCEPTANCE.md#e2e-12) | Planned |
| REQ-25.01 | Keep rescue off by default and exclude it from bulk enablement | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [047](PLAN.md#pr-047) | [13](ACCEPTANCE.md#e2e-13) | Planned |
| REQ-25.02 | Limit authorized local rescue to named actions, with validated targets and audit | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [047](PLAN.md#pr-047) | [13](ACCEPTANCE.md#e2e-13) | Planned |
| REQ-25.03 | Reject arbitrary commands, unauthorized targets and actions after revocation | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [047](PLAN.md#pr-047) | [13](ACCEPTANCE.md#e2e-13) | Planned |
| REQ-26.01 | Keep monitoring data, telemetry, files and hashes local | [PRODUCT](../architecture/PRODUCT.md#section-2) | [013](PLAN.md#pr-013), [044](PLAN.md#pr-044), [045](PLAN.md#pr-045), [046](PLAN.md#pr-046) | [04](ACCEPTANCE.md#e2e-04), [06](ACCEPTANCE.md#e2e-06), [10](ACCEPTANCE.md#e2e-10), [12](ACCEPTANCE.md#e2e-12) | Planned |
| REQ-26.02 | Expose verifiable sources, uncertainty, missing and unsupported states | [DEVELOPMENT](../standards/DEVELOPMENT.md#ag-34) | [014](PLAN.md#pr-014), [040](PLAN.md#pr-040), [045](PLAN.md#pr-045) | [06](ACCEPTANCE.md#e2e-06), [09](ACCEPTANCE.md#e2e-09), [12](ACCEPTANCE.md#e2e-12) | Planned |
| REQ-26.03 | Keep external agent decisions, models and credentials outside WinStrata | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [043](PLAN.md#pr-043), [045](PLAN.md#pr-045), [046](PLAN.md#pr-046) | [06](ACCEPTANCE.md#e2e-06), [12](ACCEPTANCE.md#e2e-12) | Planned |
| REQ-27.01 | Measure 150 MB background memory and 1 percent CPU targets | [ACCEPTANCE](../roadmap/ACCEPTANCE.md#section-10) | [048](PLAN.md#pr-048) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | Planned |
| REQ-27.02 | Measure 350 MB GUI memory, 250 MB artifact and 3 s cold start | [ACCEPTANCE](../roadmap/ACCEPTANCE.md#section-10) | [048](PLAN.md#pr-048), [050](PLAN.md#pr-050) | [08](ACCEPTANCE.md#e2e-08), [16](ACCEPTANCE.md#e2e-16) | Planned |
| REQ-27.03 | Verify zero untagged writes, negligible disabled-plugin resources and I/O costs | [ACCEPTANCE](../roadmap/ACCEPTANCE.md#section-10) | [031](PLAN.md#pr-031), [048](PLAN.md#pr-048) | [04](ACCEPTANCE.md#e2e-04), [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | Planned |
| REQ-28.01 | Centralize toolchains, lockfiles and project dependency resolution | [SYSTEM](../architecture/SYSTEM.md#section-11) | [001](PLAN.md#pr-001) | [01](ACCEPTANCE.md#e2e-01), [16](ACCEPTANCE.md#e2e-16) | Workspace pins and paths verified on Linux. |
| REQ-28.02 | Centralize configuration, migration and sensitive data handling | [DEPENDENCIES](../standards/DEPENDENCIES.md#ag-40) | [001](PLAN.md#pr-001), [006](PLAN.md#pr-006) | [01](ACCEPTANCE.md#e2e-01), [16](ACCEPTANCE.md#e2e-16) | Toolchain configuration in place. Runtime configuration planned. |
| REQ-28.03 | Provide a shared development entry point and enforce repository hygiene | [DEPENDENCIES](../standards/DEPENDENCIES.md#ag-19) | [001](PLAN.md#pr-001), [002](PLAN.md#pr-002) | [16](ACCEPTANCE.md#e2e-16) | Developer entry and ignores verified. Automated gates planned. |
| REQ-29.01 | Protect main, require PRs and have the owner review and merge in order | [GIT_AND_PR](../standards/GIT_AND_PR.md#section-13) | [000](PLAN.md#pr-000), [002](PLAN.md#pr-002) | [16](ACCEPTANCE.md#e2e-16) | Native main protection verified. PR 002 check activation pending. |
| REQ-29.02 | Validate gates, defect blocking, latest candidate and postmerge regression | [GIT_AND_PR](../standards/GIT_AND_PR.md#ag-48) | [002](PLAN.md#pr-002), [050](PLAN.md#pr-050) | [16](ACCEPTANCE.md#e2e-16) | Policy and workspace checks verified locally. Hosted enforcement pending. |
| REQ-29.03 | Finish real Windows, security, performance and end-to-end acceptance | [ACCEPTANCE](../roadmap/ACCEPTANCE.md#section-15) | [049](PLAN.md#pr-049), [050](PLAN.md#pr-050) | [01–16](ACCEPTANCE.md#e2e-01) | Planned |
| REQ-30.01 | Maintain linked bilingual standards and provenance records | [DOCUMENTATION](../standards/DOCUMENTATION.md#section-12) | [000](PLAN.md#pr-000) | [16](ACCEPTANCE.md#e2e-16) | Planned |
| REQ-30.02 | Track every requirement, all 25 plugins and 61 rules with evidence | [DOCUMENTATION](../standards/DOCUMENTATION.md#ag-61) | [000](PLAN.md#pr-000), [050](PLAN.md#pr-050) | [16](ACCEPTANCE.md#e2e-16) | Planned |

## Official plugins

All plugins are planned and await runtime evidence. See the [official plugin contract](../architecture/PRODUCT.md#section-4) for responsibilities.

| ID | Component | Plan PR | Verify |
|---|---|---|---|
| PL-01 | StrataPulse | [024](PLAN.md#pr-024) | [08](ACCEPTANCE.md#e2e-08) |
| PL-02 | StrataNet | [031](PLAN.md#pr-031) | [04](ACCEPTANCE.md#e2e-04) |
| PL-03 | StrataTrace | [021](PLAN.md#pr-021) | [02](ACCEPTANCE.md#e2e-02), [03](ACCEPTANCE.md#e2e-03) |
| PL-04 | StrataApps | [035](PLAN.md#pr-035) | [03](ACCEPTANCE.md#e2e-03), [09](ACCEPTANCE.md#e2e-09) |
| PL-05 | StrataRegistry | [019](PLAN.md#pr-019) | [02](ACCEPTANCE.md#e2e-02) |
| PL-06 | StrataEnv | [020](PLAN.md#pr-020) | [02](ACCEPTANCE.md#e2e-02) |
| PL-07 | StrataProcess | [018](PLAN.md#pr-018) | [02](ACCEPTANCE.md#e2e-02), [03](ACCEPTANCE.md#e2e-03) |
| PL-08 | StrataServices | [022](PLAN.md#pr-022) | [03](ACCEPTANCE.md#e2e-03), [14](ACCEPTANCE.md#e2e-14) |
| PL-09 | StrataStartup | [023](PLAN.md#pr-023) | [14](ACCEPTANCE.md#e2e-14) |
| PL-10 | StrataSecurity | [037](PLAN.md#pr-037) | [14](ACCEPTANCE.md#e2e-14) |
| PL-11 | StrataSnapshot | [038](PLAN.md#pr-038) | [07](ACCEPTANCE.md#e2e-07), [09](ACCEPTANCE.md#e2e-09) |
| PL-12 | StrataRules | [039](PLAN.md#pr-039) | [07](ACCEPTANCE.md#e2e-07) |
| PL-13 | StrataHealth | [025](PLAN.md#pr-025) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) |
| PL-14 | StrataTimeline | [041](PLAN.md#pr-041) | [09](ACCEPTANCE.md#e2e-09) |
| PL-15 | StrataBudget | [026](PLAN.md#pr-026) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) |
| PL-16 | StrataMCP | [045](PLAN.md#pr-045) | [12](ACCEPTANCE.md#e2e-12) |
| PL-17 | StrataRescue | [047](PLAN.md#pr-047) | [13](ACCEPTANCE.md#e2e-13) |
| PL-18 | StrataFilter | [028](PLAN.md#pr-028) | [04](ACCEPTANCE.md#e2e-04), [08](ACCEPTANCE.md#e2e-08) |
| PL-19 | StrataDisk | [032](PLAN.md#pr-032) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) |
| PL-20 | StrataScript | [027](PLAN.md#pr-027) | [07](ACCEPTANCE.md#e2e-07), [12](ACCEPTANCE.md#e2e-12) |
| PL-21 | StrataDependency | [034](PLAN.md#pr-034) | [03](ACCEPTANCE.md#e2e-03), [09](ACCEPTANCE.md#e2e-09) |
| PL-22 | StrataTrust | [033](PLAN.md#pr-033) | [03](ACCEPTANCE.md#e2e-03), [09](ACCEPTANCE.md#e2e-09) |
| PL-23 | StrataUpdate | [036](PLAN.md#pr-036) | [14](ACCEPTANCE.md#e2e-14) |
| PL-24 | StrataTag | [029](PLAN.md#pr-029) | [04](ACCEPTANCE.md#e2e-04), [05](ACCEPTANCE.md#e2e-05) |
| PL-25 | StrataStorage | [030](PLAN.md#pr-030) | [05](ACCEPTANCE.md#e2e-05) |

## 61 agent rules

The [AG-01 through AG-61 index](../../AGENTS.md) identifies each governing requirement. PR [000](PLAN.md#pr-000) reviews the full bilingual mapping. Subsequent PRs maintain rules and verify evidence under [E2E-16](ACCEPTANCE.md#e2e-16).
