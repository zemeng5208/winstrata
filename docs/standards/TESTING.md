# Testing standard

[简体中文](../zh-CN/standards/TESTING.md) · [Index](../INDEX.md)

<a id="ag-15"></a>

## AG-15 Real environments and integrated testing

Distinguish development, test, and real user environments. Prefer isolated VMs or dedicated Windows for high-risk driver, registry, network, and rescue testing. Dangerous tests on a real computer require explicit permission. Prioritize reusable real user workflows, integration, end-to-end, longevity, and performance. Use valuable unit and contract tests as needed. Synthetic successes, excessive mocks or stubs, low-value coverage, mechanical tests, and disposable test scripts cannot stand in for real validation.

<a id="ag-16"></a>

## AG-16 Regression and premerge validation

Validate new and existing behavior on the actual merge candidate. Run integrated regressions for plugin cooperation, compatibility, stability, performance, preceding fixes, and removed behavior. Fix and retest confirmed failures. Update module and global records with actual results. Unverified items use the [open-item register](../records/OPEN_ITEMS.md). Confirmed defects use the [error register](../records/ERRORS.md) and are not recorded as limitations. Preserve preceding work.

<a id="ag-17"></a>

## AG-17 Defect root causes and related issues

Investigate the root cause and scan the repository for related faults, shared components, and upstream and downstream dependencies. Fix within scope and report out-of-scope findings for approval. Coordinated delegation may help. Run overall regression, update the error register and docs, and block merge until confirmed defects are resolved. [AG-63](../../AGENTS.md#ag-63) applies. Do not weaken an assertion to close a failure.

<a id="ag-18"></a>

## AG-18 Performance and resource use

Measure applicable CPU, memory, I/O, storage growth, driver work, throughput and latency, GUI and plugin overhead, startup, and size against comparable prior environments and workloads. Maintain benchmarks. Diagnose and correct material regressions or request approval. Preserve collection coverage, precision, event integrity, and measurement definitions. Prioritize real workloads and limit wasteful microbenchmarks, I/O, and device write amplification. Use the [performance targets](../roadmap/ACCEPTANCE.md#section-10), verify platform build/packaging isolation under [AG-65](../../AGENTS.md#ag-65) and [PLATFORM_ISOLATION.md](PLATFORM_ISOLATION.md#section-2), and record unimplemented areas.

<a id="ag-37"></a>

## AG-37 Real GUI acceptance

Start the actual Windows app and interact with its real backend. Verify resizing, switching, refresh, dynamic plugin lifecycle, languages, loading, empty, permissions, failure and unsupported states, longevity, responsiveness, and memory. Check upstream components, three themes, persistence, layout, and glass and transition costs. Record evidence and mark missing Windows GUI verification pending. Static screenshots, compilation, and review cannot substitute.

<a id="ag-52"></a>

## AG-52 Unverified assumptions

Record critical assumptions and their impact, method, status, and evidence separately from fact and inference. Start with small real feasibility work on Linux driver tooling, isolation, packet capture, and privileged IPC. Report failures and approve alternatives before scaling. Label proofs of concept accurately, remove unused prototypes, and synchronize architecture, standards, and roadmap.

<a id="ag-53"></a>

## AG-53 Real user scenarios

Derive acceptance from actual launch, setup, user actions, and results on real Windows, software, networks, and data. Include normal use, mistakes, permissions, unexpected exits, and resource shortages. Capture steps, expectations, actual outcomes, and issues. After repairs rerun full workflows and check function, experience, truthfulness, performance, and stability.
