# Acceptance and performance

[简体中文](../zh-CN/roadmap/ACCEPTANCE.md) · [Index](../INDEX.md)

<a id="section-10"></a>

## Performance targets

These targets require measurements of the complete development build against representative work.

| Measure | Target |
|---|---:|
| Normal background monitoring memory | ≤ 150 MB |
| Normal background monitoring total CPU | ≤ 1% |
| Normal complete GUI memory | ≤ 350 MB |
| Complete runtime artifact size | ≤ 250 MB |
| Cold start to usable UI | ≤ 3 seconds |
| Exclusive resources for a disabled plugin | Near zero |
| Persisted untagged raw packets | 0 bytes |
| Needless redundant I/O | Identify, measure, and remove where practical |

Include Core, GUI/WebView, services, plugin processes, runtimes and attributable kernel cost. Distinguish private working set, shared physical memory and committed memory and avoid double counting. Normalize CPU to total machine capacity. Record peak, average, window, I/O, amplification, store growth, throughput, latency, gaps, launch time and byte size. Measure baseline scans, high-rate capture, large snapshots, scripts and heavy analysis separately. Keep precision, scope, supported features, and loss accounting consistent. Investigate and fix or obtain approval for regressions.

Suggested reference hardware is a Windows 11 x64, eight-core, 16 GB, NVMe machine. Freeze the exact Windows build, WebView2, tooling, hardware model, workload, enabled plugins, and units in the evidence. The candidate procedure is a 10-minute stabilized window sampled every second for background and GUI workloads, repeated three times, and five cold starts after a reboot or a verified equivalent cold state. Report means, peaks, and individual cold starts. For artifact size, include all required official plugins, runtimes and shared dependencies, including externally distributed required project components. Report optional debug symbols, OS WebView2, user data, additional third-party extensions, tools, and caches separately. Verify the measurement definitions and thresholds with the owner before declaring results.

Unmeasured engineering estimates are 80–160 MB and 0.3–1.5% CPU for background operation, 180–320 MB and 1–3% for the GUI, 150–300 MB and 1–4% with all regular monitoring plugins, and higher for heavy load. Runtime size was estimated at 90–230 MB and debug builds with symbols at 300–800 MB. Consider on-demand loading and reusing OS WebView2 subject to evidence and approval. Selective capture retention reduces writes while parsing still consumes CPU.

<a id="section-15"></a>

## Real user scenarios

Use real Windows, software and scoped data. Prioritize integrated user flows, cross-plugin behavior, longevity and performance. Use targeted unit and contract tests for critical algorithms and faults. Consolidate test assets. Mark missing verification accurately. Avoid publishing sensitive inputs, large captures or raw personal logs. High-risk actions use an authorized isolated Windows environment.

| ID | Action and expected evidence |
|---|---|
| <a id="e2e-01"></a>E2E-01 | Start with approved dependencies. Select language, D or a manual location, plugins, scope, encryption and boot behavior. Confirm no automatic C drive fallback, and build the chosen baseline. |
| <a id="e2e-02"></a>E2E-02 | On a chosen disk, create, edit, move and delete files and alter test keys and PATH. Query path, process, before/after and history. Verify exclusion of unselected locations and other users' private data. |
| <a id="e2e-03"></a>E2E-03 | Install, run, update and uninstall legitimate software. Check automatic and manual sessions, process tree, service, dependency, signature, registry and environment provenance, remnants and size. Preserve shared ownership. |
| <a id="e2e-04"></a>E2E-04 | Generate real incoming and outgoing traffic on authorized network and interfaces. Check topology, connections, protocols, and attribution. Tag a sample, export captures and prove untagged raw packets never hit disk. |
| <a id="e2e-05"></a>E2E-05 | Trigger 1 GB capture and 2 GB log notices with real collected data, using lower thresholds first if useful. Decline, progress to 1 GB and 500 MB follow-up increments, restart, confirm deletion and check indexes. Exercise GUI/script thresholds and absence of deletion without confirmation. |
| <a id="e2e-06"></a>E2E-06 | Enable, disable, replace and hot-update real plugins and install community extensions. Verify pages, dependencies, permission revocation, rollback, history, resource release, isolation, and continued independent modules. |
| <a id="e2e-07"></a>E2E-07 | Write, test, run, trigger and pause real scripts with logs and faults. Create and compare manual snapshots. Verify no preset scripts, authorized automation only, and filter fallback. |
| <a id="e2e-08"></a>E2E-08 | Read actual CPU/GPU/memory and SSD counters, diagnostics, and budgets. Apply authorized load and verify delay, limits, pause, resume, failure and loss markers, writes and resources. |
| <a id="e2e-09"></a>E2E-09 | Search a real application across sources. Check timeline and zoom, playback, relation graph, origin and uncertainty, and JSON/CSV/HTML/PCAPNG exports. Observe that playback has no system effects. |
| <a id="e2e-10"></a>E2E-10 | Lock/unlock while monitoring, test encrypted backup, migration, restore and rollback. Check plugin isolation, key access and import path confinement. |
| <a id="e2e-11"></a>E2E-11 | In real Windows WebView2, exercise and persist all three themes, verify upstream components, rendering and performance, switch language, resize, drag and persist one dashboard, show honest empty, error and unsupported states, handle close and tray, exit, switch live/history and resume. |
| <a id="e2e-12"></a>E2E-12 | Connect a real local agent through MCP, query, build and install plugins, edit scripts and use authorized capabilities. Reject remote access, revoke and disable tools, confirm publication and keep monitored data local. |
| <a id="e2e-13"></a>E2E-13 | Use harmless isolated Windows targets to verify default-off rescue, temporary authorization, allowed actions, denied admin and config operations, closure, audit and true results. |
| <a id="e2e-14"></a>E2E-14 | Observe real update, service, driver, security and certificate changes in an authorized Windows environment. Keep the observer read-only and report gaps, faults and unsupported sources. |
| <a id="e2e-15"></a>E2E-15 | Run sustained, burst, driver and plugin failure, disk-full, disconnected storage, resource scarcity and unexpected-exit scenarios. Check stable recovery, loss markers, resource leaks and recursion. |
| <a id="e2e-16"></a>E2E-16 | Exercise the exact merge candidate and actual main. Check old behavior, prior fixes, tree, references, dependency resolution, config, permissions, versions, artifacts and applicable enforced gates. |
| <a id="e2e-17"></a>E2E-17 | With the official GUI still installed, install, enable, disable, update, and uninstall a real third-party GUI plugin, then restore the default interface. Change theme, color, material, fonts, motion, interaction, layout, navigation, sidebar, header, and dashboard. Replace a shared component and another plugin's presentation, then replace the whole interface. Background monitoring continues, unrelated services stay up, and stored data remains. One complete GUI renderer runs. The plugin is managed in the existing market. |
| <a id="e2e-18"></a>E2E-18 | From that third-party GUI, attempt to bypass permission, isolation, rescue authorization, and deletion confirmation. The trusted host performs those interactions. A forged authorization is rejected. |
| <a id="e2e-19"></a>E2E-19 | Every official GUI and official plugin component or layout file in the commit has a source-register row. The upstream repository, revision, source path, license, reuse method, and modifications match the file. A missing upstream component is an owner-approved gap. A visual rewrite without that approval fails. |

E2E-11 is unchanged and remains the official-default-GUI scenario. E2E-17, E2E-18, and E2E-19 have no roadmap row. This change does not make plan 015 or plan 016 wait for them.

<a id="gui-proposal"></a>

## Replaceable-GUI proposal

Before implementation of E2E-17 or E2E-18, a development agent researches and verifies a technical approach and the owner approves it. This acceptance plan does not select that approach. The candidates already named in the system architecture remain unapproved. No roadmap row is added for the proposal or the implementation.

Each record identifies commit, OS and Windows version, hardware, driver and tools, actual command and steps, expected and observed outcome, error, rerun, and evidence. Test changes and existing behavior in the available environment. Required Windows driver and GUI checks stay pending without a Windows environment. Record unrun platforms explicitly. Verify actual main and finish phase acceptance after merges.

<a id="section-20"></a>

## Development build completion

1. Verify all 25 approved native capabilities with opt-in enablement, independent lifecycle and default-off temporary rescue.
2. Verify shared Core, collection, storage, permissions, runtimes, GUI, market, scripts and local MCP with real data and local privacy. Validate the specified frontend sources, three themes, behavior and performance. E2E-17, E2E-18, and E2E-19 stay open until the owner assigns them a roadmap row.
3. Establish Windows 10/11 x64 operation and both development/build capabilities with separate evidence. Record visibility limits.
4. Verify selective raw capture, both warning ladders, authorized cleanup, manual storage fallback, self-filtering, I/O and resource budgets, integrity, encryption, backup and migration.
5. Pass real usability, integrated regression, sustained stability, security and performance without open confirmed defects. Retain evidence.
6. Reconcile every atomic requirement, all 64 rules, records, files and documentation. The owner merges in order and reviews the verified release. AG-64 counts only after the owner merges it.
