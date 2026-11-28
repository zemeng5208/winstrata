# PR roadmap

[简体中文](../zh-CN/roadmap/PLAN.md) · [Index](../INDEX.md)

<a id="section-14"></a>

## 55 proposed PRs

The owner reviews the proposed order in PR 000. Plan numbers are independent of GitHub PR numbers. On 2026-11-28 the owner inserted plan 052 and plan 051 between 013 and 014. The same day the owner assigned plan 053 immediately after 014 and before 015, and plan 055 immediately after 030 and before 031. Plan 055's direct prerequisite is 030 only. Plans 000 through 014, 052, 051, 053, and 015 through 030 keep their relative order, and plans 031 through 050 keep theirs. Plan 015 still precedes plan 016. The dependency cells of 015 and 016 do not list 053, and the dependency cell of 031 does not list 055. Table order makes 015 wait until 053 is merged, 016 wait behind 015, and 031 wait until 055 is merged. Dependencies listed are direct and also imply their ancestors. Use named component paths and directly affected documents. Seek approval for scope expansion, unapproved interfaces or dependencies, or additions, reordering, splits and combinations. Every PR requests a merge position under the [Git standard](../standards/GIT_AND_PR.md#section-13) and applies the [acceptance plan](ACCEPTANCE.md). Confirm interfaces, exact test scenarios, and measurable criteria before implementing affected work. Record evidence for each scope during review.

### Phase 0. Foundation and feasibility

| PR | Scope | Direct prerequisites | Acceptance evidence |
|---|---|---|---|
| <a id="pr-000"></a>000 | Project foundation, AGENTS, docs, contribution policies and license | Initial baseline | Check all requirements and 65 rules, links and bilingual consistency, then obtain owner approval. AG-64 and AG-65 are proposed after PR 000 and are not accepted by the already merged plan 000. |
| <a id="pr-001"></a>001 | Cargo and pnpm, toolchains, developer entry point and project dependency paths | 000 | Initialize and build in the available environment, detect missing dependencies and verify resolution after moving the checkout. Keep outputs out of Git. |
| <a id="pr-002"></a>002 | .github, merge-order requests, main protection and shared checks | 001 | Exercise order and defect blocking, protection against bypass and reuse of the checks. |
| <a id="pr-003"></a>003 | Linux to Windows x64 cross-build and driver proof of concept | 001 | Link and inspect real artifacts and verify signing, load and security feasibility. Record blockers and distinguish the proof of concept. |
| <a id="pr-004"></a>004 | Feasibility of native sources, isolation, MCP and GUI | 001 | Try real candidates, report reuse and selection evidence, and evaluate Amicro, Glasswork, and shadcn-admin under [§9](../architecture/FRONTEND.md#section-9) for licenses, performance, and WebView2. UI Moments, designated on 2026-11-27, has not received the same verification. |

Accept after PR 000 is in effect, enforced gates and feasibility risks are understood, and critical unknowns are resourced. Pause affected work and seek approval if Linux driver builds or sandbox feasibility fail.

### Phase 1. Infrastructure and real collection

| PR | Scope | Direct prerequisites | Acceptance evidence |
|---|---|---|---|
| <a id="pr-005"></a>005 | protocols for event, object, error and version contracts | 002,003,004 | Verify real examples, version and error compatibility, and source and gap semantics. |
| <a id="pr-006"></a>006 | core config, paths, current-user identity and run state | 005 | Test real permissions, migration prerequisites, manual storage fallback and user isolation. [Implementation and verification scope](../architecture/CORE-STORAGE.md) |
| <a id="pr-007"></a>007 | core bounded scheduling, bus, backpressure and minimum resource protection | 005 | Measure real event throughput and queues, cancel tasks, record gaps and check for leaks. |
| <a id="pr-008"></a>008 | services and kernel for controlled IPC, privileges and driver management | 005,006,007 | Test Windows load and unload, identity, permissions, and fault handling with the GUI unprivileged. |
| <a id="pr-009"></a>009 | kernel and core for file and registry collection | 008 | Observe real file and registry changes with source, accuracy, stability and on-demand loading. |
| <a id="pr-010"></a>010 | kernel and core for processes, threads, modules and system sources | 008 | Test process trees and PID lifecycle, indirect service events and deduplication. |
| <a id="pr-011"></a>011 | kernel and core for network sources, interfaces and attribution | 008 | Observe real inbound/outbound traffic, interface differences, encryption and gaps. Verify selective persistence behavior. |
| <a id="pr-012"></a>012 | core shared structured, event, metric, snapshot and capture storage | 006,007 | Write and query actual data, segment and index it, measure capacity, and test corruption and rollback. |
| <a id="pr-013"></a>013 | core encryption, backup, restore, migration and data isolation | 012 | Verify passwords and locking, encryption and decryption, cross-location and cross-device integrity. |
| <a id="pr-052"></a>052 | core process-tree exit sample | 013 | The real process-tree test reports one exit only after the child leaves a complete sample. A visible PID without a start time is not an exit. Plan 051 does not implement this fix. |
| <a id="pr-051"></a>051 | Standards and records: AG-62, AG-63, the UI Moments designation, and the open-item and error backfill | 052 | Owner approval of the new rule text. The bilingual registers match plans 000 through 013. UI Moments stays unverified. This plan does not fix ERR-013. |
| <a id="pr-014"></a>014 | core and runtimes plugin contracts, dependencies and lifecycle | 005,007,008,012 | Load, disable, update and isolate real plugins and recover dependencies. |
| <a id="pr-053"></a>053 | Standards only: AG-64, bilingual architecture, requirements, acceptance, and the GUI source register | 014 | Owner approval of the AG-64 text. The bilingual documents agree. The source register records that main `0accacb` has no product GUI file. This plan does not implement apps/desktop and does not add itself to the dependency cells of 015 or 016. |
| <a id="pr-015"></a>015 | runtimes and sdk for Python/TS/JS and isolation | 004,014 | Run real scripts and plugins, test revocation and boundary enforcement, release resources and check project-level dependencies. |
| <a id="pr-016"></a>016 | apps/desktop and packages for GUI, navigation, languages, tray, IPC, upstream UI and three themes | 006,014 | Use real Windows GUI and tray under ordinary privileges. Verify theme persistence, reuse and adaptation, performance and licenses. This row accepts the official default GUI only. E2E-17, E2E-18, and E2E-19 are not this acceptance. This row's place, after 015 and before 017, is unchanged. |
| <a id="pr-017"></a>017 | core and apps for onboarding, enablement, baseline, modes and scope | 009,010,012,014,016 | Run real onboarding and verify scope, pause/resume, live/history and loss markers. |

Once contracts are approved, independent plugin preparation can proceed under the merge order. Demonstrate an actual collection-to-storage-to-GUI or query flow and verify lifecycle and security.

### Phase 2. Monitoring and data protection

| PR | Scope | Direct prerequisites | Acceptance evidence |
|---|---|---|---|
| <a id="pr-018"></a>018 | plugins/strata-process | 010,014,016 | Verify real process, thread, module, tree and provenance events and release on disable. |
| <a id="pr-019"></a>019 | plugins/strata-registry | 009,014,016 | Verify before/after on real current-user and relevant system keys and values with provenance. |
| <a id="pr-020"></a>020 | plugins/strata-env | 019 | Observe real user and relevant system environment and PATH changes. |
| <a id="pr-021"></a>021 | plugins/strata-trace | 009,010,017,018,019 | Verify file changes, permissions, cross-source events, paths and evidence without content versions. |
| <a id="pr-022"></a>022 | plugins/strata-services | 010,018 | Observe actual service and driver states, changes, permissions and failures. |
| <a id="pr-023"></a>023 | plugins/strata-startup | 019,022 | Observe real startup and task changes, attribution and history. |
| <a id="pr-024"></a>024 | plugins/strata-pulse | 010,012,014,016 | Check CPU/GPU/memory/hardware counters, history and unsupported readings. |
| <a id="pr-025"></a>025 | plugins/strata-health | 007,008,012,014,016 | Verify own diagnostic data for faults, queues, latency and loss. |
| <a id="pr-026"></a>026 | plugins/strata-budget | 024,025 | Simulate real resource pressure, adjust limits, pause and resume, and verify loss marking. |
| <a id="pr-027"></a>027 | plugins/strata-script | 015,016 | Use the editor, three languages, debugging, triggers, logs, disablement and permissions. Verify empty presets. |
| <a id="pr-028"></a>028 | plugins/strata-filter | 021,025,027 | Prevent self-event recursion and preserve outside changes. Test GUI and script filtering, batch writes and I/O. |
| <a id="pr-029"></a>029 | plugins/strata-tag | 012,014,016,027 | Tag real events manually and by scripts with evidence, permissions and attribution. Verify the untagged packet policy. |
| <a id="pr-030"></a>030 | plugins/strata-storage | 012,013,027,029 | Trigger, decline and repeat thresholds across restart. Confirm deletion, indexes and GUI/script configuration. |
| <a id="pr-055"></a>055 | Standards only: AG-65, bilingual PLATFORM_ISOLATION.md, mandatory reading entry, and linked governance records | 030 | Owner approval of the AG-65 text and PLATFORM_ISOLATION.md. Bilingual standards match baseline `4f257ae`. This plan keeps existing Linux reserves, makes no runtime code changes, and does not add itself to the dependency cell of 031. |
| <a id="pr-031"></a>031 | plugins/strata-net | 011,017,018,026,028,029,030 | Verify actual capture, traffic, topology, authorized discovery, risk-only writes and PCAPNG. |
| <a id="pr-032"></a>032 | plugins/strata-disk | 024,025,028,030 | Check real drive health and I/O, logical/physical distinction, unsupported counters and write impact. |

Verify real process, file, registry, environment, service, task, network, selective retention, scripts, data management, and budgets together. Resolve confirmed defects.

### Phase 3. Software lifecycle and correlation

| PR | Scope | Direct prerequisites | Acceptance evidence |
|---|---|---|---|
| <a id="pr-033"></a>033 | plugins/strata-trust | 018,021,028 | Verify actual signatures, certificates, integrity and origin. Keep hashes local and show unsigned status objectively. |
| <a id="pr-034"></a>034 | plugins/strata-dependency | 018,021,022,033 | Trace real DLL, runtime and shared dependencies, conflicts, and ownership evidence. |
| <a id="pr-035"></a>035 | plugins/strata-apps | 019,020,021,022,023,032,033,034 | Install, run, update and uninstall real software. Check manual sessions, reconstructed history, remnants and size attribution. |
| <a id="pr-036"></a>036 | plugins/strata-update | 021,022,035 | Observe Windows updates, features, drivers, errors and related changes read-only. |
| <a id="pr-037"></a>037 | plugins/strata-security | 019,022,023,031,033 | Observe actual security configuration, permissions and certificate events and report objective state. |
| <a id="pr-038"></a>038 | plugins/strata-snapshot | 012,013,017,021,027 | Verify real full and incremental manual snapshots, differences, import/export, default-off automation and no presets. |
| <a id="pr-039"></a>039 | plugins/strata-rules | 021,024,027,029,031 | Check user rules, correlations, thresholds, notifications, permissions and read-only behavior. |
| <a id="pr-040"></a>040 | core and apps for shared index, profiles, search and relations | 031,035,036,037,038 | Search real software across sources and check permissions, provenance, latency and attribution. |
| <a id="pr-041"></a>041 | plugins/strata-timeline | 040 | Use multiple timelines, historical playback, gaps, confidence, source jumps and export on real data. |
| <a id="pr-042"></a>042 | apps and core for one customizable dashboard, full flows and exports | 016,031,035,039,040,041 | Exercise drag and save, language changes, dynamic widgets, real JSON/CSV/HTML exports and history mode. |

Verify the whole software lifecycle, cross-plugin analysis, snapshots, timelines and GUI in Windows. Build in the available environment and check unsupported states and observable loss.

### Phase 4. Extensions and local agents

| PR | Scope | Direct prerequisites | Acceptance evidence |
|---|---|---|---|
| <a id="pr-043"></a>043 | core and apps for shared market, composite packages and management views | 015,027,042 | Install, disable, uninstall, change permissions, manage dependency conflicts, use local and public indexes and check publication rules. |
| <a id="pr-044"></a>044 | core and services for versioning, signed updates, pinning and transactional rollback | 013,014,043 | Verify updates, compatibility and failure rollback with data and settings intact, and prompt for driver restarts. |
| <a id="pr-045"></a>045 | plugins/strata-mcp local connection and queries | 040,041,043 | Connect a real local agent, update tools, revoke, disable, reject remote calls and query history. |
| <a id="pr-046"></a>046 | MCP plugin/script development and publication workflows | 027,043,044,045 | Have an external agent create, edit, build, install, reload, test and package real extensions. Verify publication confirmation and credential isolation. |
| <a id="pr-047"></a>047 | plugins/strata-rescue and services temporary restricted channel | 008,037,045 | Exercise authorized actions against harmless isolated Windows targets. Verify default off, remote and unauthorized denial, revocation and auditing. |

Verify an actual extension ecosystem, external local agents, authorization, publication and rescue boundaries. Ship every built-in with opt-in activation and verify resource release.

### Phase 5. Development build delivery

| PR | Scope | Direct prerequisites | Acceptance evidence |
|---|---|---|---|
| <a id="pr-048"></a>048 | Approved performance, I/O, storage and sustained stability work | 042,044,046,047 | Compare actual benchmarks and budgets, preserve coverage, study errors and quantify hardware overhead. |
| <a id="pr-049"></a>049 | Approved security, compatibility and real user regression fixes | 048 | Validate the supported Windows 10/11 range, driver safeguards, MCP, plugins, imports, encryption and real end-to-end flows. |
| <a id="pr-050"></a>050 | Development build delivery, docs, SDK, reproducibility and acceptance | 049 | Check every requirement, 25 plugins, 65 rules, available-host verification, both build capabilities, platform build/packaging isolation (`OI-061`), real artifacts and debug material. Obtain final approval. AG-64 and AG-65 are included once the owner has merged them. Requirements with no roadmap row stay open until the owner assigns one. |

Address findings from real integrated validation and approved optimizations. Resolve earlier confirmed defects before their next merge. Ask for a roadmap amendment if a planned PR has no work. Installer and portable releases are outside this phase.


<a id="section-19"></a>

## Open decisions and feasibility risks

| Area | Decision or evidence needed |
|---|---|
| Repository and name | The owner selected and created zemeng5208/winstrata. Another software business uses winstrata.com. Formal trademark verification is pending. |
| Frontend | Specified sources are Amicro, Glasswork, shadcn-admin, and UI Moments. Three themes remain. The plan 004 report verifies only the first three; UI Moments has not received the same verification. Verify version, coverage, license text, WebView2, resource cost, and initial theme before integration. Designation does not approve Fluent UI or Astryx as product dependencies. AG-64 is plan 053, the standards row immediately after 014. That row does not implement the GUI and does not add another plan ID for the replacement work. |
| Native Linux Windows build | PR 003 verifies WDK, linking, signing, and Windows loading under actual protections. |
| Runtime and isolation | PRs 004 and 015 verify isolation and overhead before approving packages. |
| Windows and interfaces | Validate supported Windows 10/11 x64 versions and observable devices. |
| Performance | Review representative workloads, averages and peaks, accounting, and exact hardware. |
| Untagged connection metadata | Decide whether to persist it. Untagged raw packets remain memory-only. |
| Capacity and deletion | Confirm units, saved log accounting, default cleanup selection and expiration authorization. Agreed initial and repeat warning levels still apply. |
| Dependencies | Evaluate and approve runtime, SDK, reuse sources and versions before adoption. |
| Roadmap | Owner approval covers PR order, path organization, and specific acceptance. Actual CI status belongs in the progress record. |
