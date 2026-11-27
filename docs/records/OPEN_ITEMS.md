# Open-item register

[简体中文](../zh-CN/records/OPEN_ITEMS.md) · [Errors](ERRORS.md) · [Progress](STATUS.md) · [Index](../INDEX.md)

This register is required by [AG-62](../../AGENTS.md#ag-62). Every still-open item from plans 000 through 013 that the engineering record, acceptance plan, or architecture notes left without these fields is listed here. [STATUS.md](STATUS.md) is not a substitute. Confirmed defects are errors, not rows in this file. [ERR-005](modules/core.md#err-005) was retested on the plan 052 merge.

The handoff owner for every row is repository owner zemeng5208. The developer of the responsible plan runs the named environment and records the result. The owner accepts the close. “Awaiting verification” is not a close condition.

Plan 052 is merged and is the process-exit fix. Plan 051 writes this register and does not close these items. Plan 014 is merged as `0accacbaa2dac43db3845cc8c08f23b4e831c413`. Plan 053 records AG-64 immediately after 014 and does not close these items.

## Summary

| ID | State | Responsible plan |
|---|---|---|
| [OI-001](#oi-001) | Untested | 049 |
| [OI-002](#oi-002) | Untested | 049 |
| [OI-003](#oi-003) | Untested | 049 |
| [OI-004](#oi-004) | Untested | 049 |
| [OI-005](#oi-005) | Untested | 042 |
| [OI-006](#oi-006) | Untested | 031 |
| [OI-007](#oi-007) | Unaccepted | 008, then 050 for the Linux cross-build |
| [OI-008](#oi-008) | Known limitation | Later storage plan, not 051 |
| [OI-009](#oi-009) | Untested | 016 |
| [OI-010](#oi-010) | Partially tested | 050 |
| [OI-011](#oi-011) | Untested | 050 |
| [OI-012](#oi-012) | Untested | 050 |
| [OI-013](#oi-013) | Untested | 016 |
| [OI-014](#oi-014) | Unaccepted | 008 follow-up |
| [OI-015](#oi-015) | Known limitation | 018, 022 |
| [OI-016](#oi-016) | Known limitation | 019, 021 |
| [OI-017](#oi-017) | Untested | 031 |
| [OI-018](#oi-018) | Untested | 021 |
| [OI-019](#oi-019) | Partly tested | 015 |
| [OI-020](#oi-020) | Untested | 015, 048 |
| [OI-021](#oi-021) | Known limitation | 050 |
| [OI-022](#oi-022) | Pending decision | Owner |
| [OI-023](#oi-023) | Pending decision | 016 |
| [OI-024](#oi-024) | Pending decision | 030 |
| [OI-025](#oi-025) | Pending decision | Owner, before 031 persists it |
| [OI-026](#oi-026) | Pending decision | 048 |
| [OI-027](#oi-027) | Pending decision | 049 |
| [OI-028](#oi-028) | Recorded constraint | Stays until a dependency approval |
| [OI-029](#oi-029) | Known limitation | 014, 015 |
| [OI-030](#oi-030) | Recorded constraint | Stays until a dependency approval |
| [OI-031](#oi-031) | Recorded constraint | 038, 042 for REQ-14.03 |
| [OI-032](#oi-032) | Evidence gap | Next records update |
| [OI-033](#oi-033) | Open process item | Owner closes GitHub PR #11 |
| [OI-034](#oi-034) | Known limitation | 031 |
| [OI-035](#oi-035) | Known limitation | 016 |
| [OI-036](#oi-036) | Register matches this commit; E2E-19 still open | 016, without a new predecessor |
| [OI-037](#oi-037) | Unimplemented | No roadmap row |
| [OI-038](#oi-038) | Pending decision | Owner |
| [E2E-01](#e2e-01) through [E2E-16](#e2e-16) | Not done | See each row |
| [E2E-17](#e2e-17) through [E2E-19](#e2e-19) | Not done | No roadmap row |

<a id="oi-001"></a>

## OI-001

- Related requirement: [REQ-29.03](../roadmap/REQUIREMENTS.md)
- Source PR: plans 004 and 006 through 013. The recorded Windows jobs are Windows Server 2022.
- Current state: untested
- Reason it is incomplete: no Windows 10 or Windows 11 client run is recorded. Server 2022 success is not that run.
- Responsible PR: 049
- Handoff owner: repository owner zemeng5208
- Verification environment: Windows 10 x64 and Windows 11 x64 client installations, not Server 2022
- Acceptance criteria: the workspace tests that exist at that time, and the product flows that exist, run on both client versions and the versions are recorded
- Close conditions: those records exist, or the owner records a narrower supported range from evidence. Not closed by a Server 2022 job.

<a id="oi-002"></a>

## OI-002

- Related requirement: [REQ-11.01](../roadmap/REQUIREMENTS.md)
- Source PR: plans 006, 012, and 013. [Core storage](../architecture/CORE-STORAGE.md) says ReFS has not been exercised.
- Current state: untested
- Reason it is incomplete: filesystem testing that was recorded used NTFS on Windows and the Linux host filesystem
- Responsible PR: 049
- Handoff owner: repository owner zemeng5208
- Verification environment: a Windows volume formatted ReFS
- Acceptance criteria: private-directory checks, migration, and hybrid create are exercised on ReFS, or the attempt fails and the limitation is recorded
- Close conditions: that evidence is in the repository. Not closed by NTFS results.

<a id="oi-003"></a>

## OI-003

- Related requirement: [REQ-14.02](../roadmap/REQUIREMENTS.md)
- Source PR: plan 013. The plan 013 acceptance text includes cross-device integrity. A second physical device was not run.
- Current state: untested
- Reason it is incomplete: Linux cross-location migration keeps the source on the same host. It is not a second machine.
- Responsible PR: 049. Plan 038 remains plugin and time-range export. Plan 051 does not run the device.
- Handoff owner: repository owner zemeng5208
- Verification environment: two physical machines, using the portable vault
- Acceptance criteria: ciphertext is restored or migrated onto the second machine and decrypts there, and the source remains as the plan 013 rule requires
- Close conditions: that run is recorded. Not closed by the same-host Linux migration.

<a id="oi-004"></a>

## OI-004

- Related requirement: [REQ-14.02](../roadmap/REQUIREMENTS.md)
- Source PR: plan 013
- Current state: untested
- Reason it is incomplete: a second OS account was not run. The Linux user-mismatch test is not that account.
- Responsible PR: 049
- Handoff owner: repository owner zemeng5208
- Verification environment: two OS accounts on one host
- Acceptance criteria: migration or restore across the two accounts is executed and the user check is recorded
- Close conditions: that run is recorded

<a id="oi-005"></a>

## OI-005

- Related requirement: [REQ-14.01](../roadmap/REQUIREMENTS.md)
- Source PR: plan 013. GUI lock display remains plan 042.
- Current state: untested
- Reason it is incomplete: no GUI was run
- Responsible PR: 042
- Handoff owner: repository owner zemeng5208
- Verification environment: the Windows GUI on a machine that can lock and unlock the vault
- Acceptance criteria: the lock state is visible, a locked read is refused, and a live commit still follows the plan 013 rule
- Close conditions: that GUI run is recorded. Not closed by the Core tests alone.

<a id="oi-006"></a>

## OI-006

- Related requirement: [REQ-11.02](../roadmap/REQUIREMENTS.md)
- Source PR: plan 012. The host had no `tshark`, `tcpdump`, or `editcap`.
- Current state: untested
- Reason it is incomplete: the in-tree parser checked section-header magic `0x0A0D0D0A` and read the packet bytes back. No external PCAPNG tool was run.
- Responsible PR: 031
- Handoff owner: repository owner zemeng5208
- Verification environment: a host with one of those three tools
- Acceptance criteria: the tool reads a risk-tagged capture written by the store
- Close conditions: the tool name, version, and result are recorded. The in-tree parser does not close this item.

<a id="oi-007"></a>

## OI-007

- Related requirement: [AS-001](STATUS.md) and plan 008 acceptance, which requires Windows load and unload
- Source PR: plans 003, 008, 009, 010, and 011. No signed INF/CAT package was built or loaded. Secure Boot and HVCI were not exercised. `CatalogHost` returns `image_absent`.
- Current state: unaccepted
- Reason it is incomplete: Linux linked the plan 003 probes. Windows jobs refused to start a missing image. That is not a signed load.
- Responsible PR: the unfinished plan 008 acceptance. Plan 050 still has to verify the full Linux cross-build and does not by itself close the Windows load. The owner assigns the driver PR. Plan 051 does not load a driver.
- Handoff owner: repository owner zemeng5208
- Verification environment: Windows with Secure Boot and HVCI enabled, and a signed INF/CAT package
- Acceptance criteria: a representative driver loads and unloads, and the evidence names the package, the Windows edition, and the protection state
- Close conditions: that evidence is recorded. A `ready` result from a test host does not close it. Not marked passed.

<a id="oi-008"></a>

## OI-008

- Related requirement: [REQ-14.01](../roadmap/REQUIREMENTS.md)
- Source PR: plan 013. [Core storage](../architecture/CORE-STORAGE.md) says a crash between the segment rewrite and the vault rewrap can leave new ciphertext under the old wrap. If rewrap fails in-process, Core tries to write the previous key back. This is not a crash-safe rotation.
- Current state: known limitation
- Reason it is incomplete: the crash window is described and was not closed. It is not accepted.
- Responsible PR: a later storage plan after plan 052. Not plan 051, and not plan 014's plugin work.
- Handoff owner: repository owner zemeng5208
- Verification environment: fault injection between the rewrite and the rewrap, on Linux and on Windows Server 2022
- Acceptance criteria: that crash does not leave new ciphertext under the old wrap, and the retest is recorded
- Close conditions: the retest passes on both environments. DPAPI stays unused under [OI-031](#oi-031). [REQ-14.03](../roadmap/REQUIREMENTS.md) stays in plans 038 and 042.

<a id="oi-009"></a>

## OI-009

- Related requirement: [REQ-19.01](../roadmap/REQUIREMENTS.md)
- Source PR: owner designation on 2026-11-27, recorded by plan 051. Plan 004 measured Amicro, Glasswork, and shadcn-admin only. Upstream commit read for the designation: `c9eece0fe425ee7dfe97d162d39d1665c3bf9368`.
- Current state: untested
- Reason it is incomplete: Plan 016 mounts the original Ink Switch and Signal Return from revision `c9eece0fe425ee7dfe97d162d39d1665c3bf9368`. Fluent UI, Astryx, and the Geist font file are not product files. This revision still has not had the plan 004 WebView2 or performance measurement.
- Responsible PR: 016
- Handoff owner: repository owner zemeng5208
- Verification environment: the same class of offline WebView2 measurement used for the three measured sources, plus the selected revision's license and version review
- Acceptance criteria: that evidence is recorded for the selected UI Moments revision, or the owner withdraws the designation
- Close conditions: one of those two outcomes is recorded. Designation does not close it. GitHub PR #11 stays open for the owner. See [OI-033](#oi-033).

<a id="oi-010"></a>

## OI-010

- Related requirement: [REQ-28.03](../roadmap/REQUIREMENTS.md)
- Source PR: plans 007 through 013. The active Node.js on those hosts was v22.23.4. The pinned engine is 24.22.3. `cargo dev doctor` was not run there. Plans 012 and 013 also did not run `cargo dev verify` on that host. The workspace test command was run directly.
- Current state: partially tested
- Reason it is incomplete: Linux CI runs `.github/scripts/check-workspace.sh`, which calls `cargo dev verify`, and verify calls the doctor check. Windows CI runs `cargo test --locked --workspace` and does not run `cargo dev doctor`. The mismatched local Node runs are not a pinned-Node doctor result.
- Responsible PR: 050
- Handoff owner: repository owner zemeng5208
- Verification environment: Windows with Node.js 24.22.3 and the pinned Rust toolchain
- Acceptance criteria: `cargo dev doctor` and `cargo dev verify` are recorded on that Windows host
- Close conditions: that record exists. Linux CI does not close the Windows doctor gap.

<a id="oi-011"></a>

## OI-011

- Related requirement: [REQ-28.01](../roadmap/REQUIREMENTS.md)
- Source PR: plan 001. Linux doctor, init, build, and verify passed. Windows host execution remains unverified.
- Current state: untested
- Reason it is incomplete: no Windows run of the developer tool was recorded
- Responsible PR: 050
- Handoff owner: repository owner zemeng5208
- Verification environment: Windows x64 with the pinned toolchain
- Acceptance criteria: `cargo dev doctor`, `cargo dev init`, `cargo dev build`, and `cargo dev verify` pass on that host, or the failures are recorded as errors
- Close conditions: the command results are recorded

<a id="oi-012"></a>

## OI-012

- Related requirement: plan 003 acceptance, link and inspect real artifacts
- Source PR: plan 003. The Linux probes linked. No Windows program or driver was executed. Images stayed in ignored directories.
- Current state: untested
- Reason it is incomplete: the probe images were not executed on Windows. Signing and load remain [OI-007](#oi-007).
- Responsible PR: 050 for the cross-build evidence. OI-007 remains the load.
- Handoff owner: repository owner zemeng5208
- Verification environment: a Windows host that can inspect the linked PE images without treating them as a product driver
- Acceptance criteria: the images are inspected on Windows, or the owner records why the Linux inspection remains the only evidence
- Close conditions: that record exists. It does not mark a driver loaded.

<a id="oi-013"></a>

## OI-013

- Related requirement: [AS-002](STATUS.md) and [E2E-11](#e2e-11)
- Source PR: plan 008. The Windows job is not an interactive unelevated GUI.
- Current state: untested
- Reason it is incomplete: Plan 016 type-checks the Windows window, tray, and WebView2 loader path. This Linux host did not run an ordinary Windows GUI session. E2E-11 is not complete.
- Responsible PR: 016
- Handoff owner: repository owner zemeng5208
- Verification environment: an ordinary-user Windows GUI session
- Acceptance criteria: the GUI runs under an unelevated user and the privilege is recorded
- Close conditions: that session is recorded

<a id="oi-014"></a>

## OI-014

- Related requirement: plan 008 acceptance for the service control plane
- Source PR: plan 008. `winstrata-service` is not registered with the Service Control Manager.
- Current state: unaccepted
- Reason it is incomplete: pipe and token tests ran. Service registration did not.
- Responsible PR: unfinished plan 008 acceptance. The owner schedules the services change. Plan 047 does not replace this item.
- Handoff owner: repository owner zemeng5208
- Verification environment: Windows Server 2022 Service Control Manager
- Acceptance criteria: `winstrata-service` is created, started, and stopped through the Service Control Manager, or the owner names another plan that owns registration before plan 008 can be called accepted
- Close conditions: one of those two records exists

<a id="oi-015"></a>

## OI-015

- Related requirement: [AS-007](STATUS.md) and [REQ-06.02](../roadmap/REQUIREMENTS.md)
- Source PR: plan 010. Sampling does not emit an event for a process that starts and exits between samples. Exit time is the sample time. ETW, kernel callbacks, command lines, and service stop records are not part of plan 010.
- Current state: known limitation
- Reason it is incomplete: Plan 018 presents the plan 010 sample through `strata.process`, including real process, thread, module, tree, and provenance events, and it joins the sampler on disable. ETW, kernel callbacks, command lines, and processes that exist only between samples were not implemented. Service stop records remain plan 022.
- Responsible PR: 022 for service stop records. ETW, command lines, and kernel callbacks stay open until the owner confirms they remain outside plan 018 or a later plan tests them.
- Handoff owner: repository owner zemeng5208
- Verification environment: Windows, with the source that plan 018 or 022 actually implements
- Acceptance criteria: the deferred source is tested, or the owner confirms it stays out of those plans
- Close conditions: that confirmation or test is recorded. Not closed by the sampler tests.

<a id="oi-016"></a>

## OI-016

- Related requirement: [AS-006](STATUS.md)
- Source PR: plan 009. `ReadDirectoryChangesW` and `RegNotifyChangeKeyValue` omit `precision_ns` and the acting process. A signed minifilter or registry callback was not loaded.
- Current state: known limitation
- Reason it is incomplete: the user-mode APIs do not report those fields. The limit does not forbid a later approved kernel callback.
- Responsible PR: 019 and 021
- Handoff owner: repository owner zemeng5208
- Verification environment: the Windows watch or a later approved kernel callback
- Acceptance criteria: before/after attribution is tested in the responsible plan, or the omitted fields stay documented
- Close conditions: the responsible plan's evidence is recorded. Not marked as kernel attribution.

<a id="oi-017"></a>

## OI-017

- Related requirement: plan 011 acceptance and [E2E-04](#e2e-04)
- Source PR: plan 011. Signed `strata-net.sys` load, topology, discovery, and PCAPNG export were not run. Untagged connection metadata is not persisted. `CatalogHost` still returns `image_absent`.
- Current state: untested
- Reason it is incomplete: user-mode observation ran. The product network experience did not.
- Responsible PR: 031. Persistence of untagged metadata also waits on [OI-025](#oi-025).
- Handoff owner: repository owner zemeng5208
- Verification environment: Windows, for the product capture path plan 031 owns
- Acceptance criteria: the plan 031 scenarios that replace these gaps are run, and a signed driver is not claimed unless [OI-007](#oi-007) is closed
- Close conditions: those results are recorded

<a id="oi-018"></a>

## OI-018

- Related requirement: plan 009 file-watch behavior
- Source PR: plan 009. A live overflow of the 16 KiB file buffer was not forced.
- Current state: untested
- Reason it is incomplete: the overflow path was not driven
- Responsible PR: 021
- Handoff owner: repository owner zemeng5208
- Verification environment: Windows file watch with a change larger than the 16 KiB buffer
- Acceptance criteria: the test forces the overflow and records the gap behavior
- Close conditions: that result is recorded. Not marked passed.

<a id="oi-019"></a>

## OI-019

- Related requirement: plan 004 isolation evidence and [REQ-21.02](../roadmap/REQUIREMENTS.md)
- Source PR: plan 004. The platform report says revocation of already-open handles, network access, the registry, and hosting full Python or Node runtimes remain unverified.
- Current state: untested on Windows
- Reason it is incomplete: Linux plan 015 denied sockets, new programs, files outside the ruleset, and inherited descriptors for real Python and Node processes. A handle opened inside the allowed state directory stayed readable after that. The Windows AppContainer, registry, and network test is in the suite and was not executed on the authoring host.
- Responsible PR: 015
- Handoff owner: repository owner zemeng5208
- Verification environment: Windows AppContainer or the isolation plan 015 selects
- Acceptance criteria: each named gap is tested or explicitly left unsupported with evidence
- Close conditions: that record exists

<a id="oi-020"></a>

## OI-020

- Related requirement: [E2E-08](#e2e-08) and [E2E-15](#e2e-15)
- Source PR: plan 007. Windows 10/11 client behavior, GUI latency, sustained pressure, dynamic budgets, authenticated IPC, ReFS, and encrypted migration were not run in that plan. Encrypted migration was later exercised by plan 013 on Linux and on Windows Server 2022 review head `36b5cd6`. The other items were not.
- Current state: untested
- Reason it is incomplete: the scheduler tests are not a sustained-load or GUI run
- Responsible PR: 015 for authenticated IPC, 048 for sustained pressure, GUI latency, and dynamic budgets
- Handoff owner: repository owner zemeng5208
- Verification environment: the environment each responsible plan names
- Acceptance criteria: the named measurement or IPC check is recorded
- Close conditions: those records exist. The 20,000-event Linux release sample does not close them.

<a id="oi-021"></a>

## OI-021

- Related requirement: [REQ-28.03](../roadmap/REQUIREMENTS.md)
- Source PR: plan 006. Workspace-wide rustfmt still reports legacy layout differences in unchanged developer-tool source.
- Current state: known limitation
- Reason it is incomplete: the difference was recorded. No failing CI check for rustfmt was stored, so this is not entered as a defect.
- Responsible PR: 050
- Handoff owner: repository owner zemeng5208
- Verification environment: the pinned Rust toolchain
- Acceptance criteria: `cargo fmt --check` is clean for `toolchains/dev`, or the owner accepts the current layout
- Close conditions: one of those two records exists

<a id="oi-022"></a>

## OI-022

- Related requirement: the repository-name row in [open decisions](../roadmap/PLAN.md#section-19)
- Source PR: plan 000
- Current state: pending decision
- Reason it is incomplete: another software business uses winstrata.com. Formal trademark verification is pending.
- Responsible PR: the owner, outside a feature plan
- Handoff owner: repository owner zemeng5208
- Verification environment: the trademark search the owner chooses
- Acceptance criteria: the formal result is recorded
- Close conditions: that result is in the repository

<a id="oi-023"></a>

## OI-023

- Related requirement: [AS-003](STATUS.md) and [REQ-19.02](../roadmap/REQUIREMENTS.md)
- Source PR: the open-decision list. The initial theme is not selected.
- Current state: pending decision
- Reason it is incomplete: Plan 016 stores no theme until the user chooses one. The owner has not confirmed an initial theme, so none is presented as the product default.
- Responsible PR: 016
- Handoff owner: repository owner zemeng5208
- Verification environment: the plan 016 GUI
- Acceptance criteria: the owner confirms the initial theme before it is presented as the default
- Close conditions: the decision is recorded

<a id="oi-024"></a>

## OI-024

- Related requirement: [AS-003](STATUS.md) and the capacity row in [open decisions](../roadmap/PLAN.md#section-19)
- Source PR: plans 000 and 012. Retention days can be stored. Cleanup is plan 030.
- Current state: pending decision
- Reason it is incomplete: units, saved-log accounting, the default cleanup selection, and expiration authorization are not confirmed
- Responsible PR: 030
- Handoff owner: repository owner zemeng5208
- Verification environment: the plan 030 cleanup flow
- Acceptance criteria: those semantics are confirmed before cleanup deletes data
- Close conditions: the decision and the plan 030 test are recorded

<a id="oi-025"></a>

## OI-025

- Related requirement: [AS-003](STATUS.md) and [REQ-10.02](../roadmap/REQUIREMENTS.md)
- Source PR: plan 011 does not persist untagged connection metadata. Untagged raw packets remain memory-only.
- Current state: pending decision
- Reason it is incomplete: whether to persist untagged connection metadata is undecided
- Responsible PR: the owner decides before plan 031 persists any of it
- Handoff owner: repository owner zemeng5208
- Verification environment: not a runtime until the decision exists
- Acceptance criteria: the decision is written into the roadmap
- Close conditions: that decision is recorded. Plan 011's refusal to persist it does not choose the long-term policy.

<a id="oi-026"></a>

## OI-026

- Related requirement: [AS-003](STATUS.md) and [REQ-27.01](../roadmap/REQUIREMENTS.md)
- Source PR: the acceptance plan gives a suggested procedure. The owner has not confirmed it.
- Current state: pending decision
- Reason it is incomplete: representative workloads, averages and peaks, accounting, and the exact hardware are still open
- Responsible PR: 048
- Handoff owner: repository owner zemeng5208
- Verification environment: the hardware named when the owner confirms the method
- Acceptance criteria: the measurement definition is confirmed before results are declared
- Close conditions: the confirmation is recorded

<a id="oi-027"></a>

## OI-027

- Related requirement: the Windows row in [open decisions](../roadmap/PLAN.md#section-19)
- Source PR: plan 000. Windows 10/11 x64 is the range. The minimum build is not confirmed.
- Current state: pending decision
- Reason it is incomplete: client runs have not produced the evidence. See [OI-001](#oi-001).
- Responsible PR: 049
- Handoff owner: repository owner zemeng5208
- Verification environment: the client versions in OI-001
- Acceptance criteria: the minimum supported build is recorded from those runs or from an owner decision
- Close conditions: that record exists

<a id="oi-028"></a>

## OI-028

- Related requirement: [AS-004](STATUS.md)
- Source PR: plan 007. The worker pool uses the Rust standard library because no async-runtime crate is approved.
- Current state: recorded constraint
- Reason it is incomplete: this is a standing constraint, not an unverified claim that an async runtime was tested
- Responsible PR: a future dependency plan, only if a replacement runtime is proposed
- Handoff owner: repository owner zemeng5208
- Verification environment: not applicable while the standard library remains the implementation
- Acceptance criteria: the constraint stays in force until a separate dependency approval
- Close conditions: an approved dependency replaces it, or it remains recorded as the current design. It is not a defect.

<a id="oi-029"></a>

## OI-029

- Related requirement: [AS-005](STATUS.md)
- Source PR: plan 008. The service authenticates the OS user, not the process role. A process running as the service owner can send role `core`.
- Current state: known limitation
- Reason it is incomplete: plugin capability separation is not implemented
- Responsible PR: 014 and 015
- Handoff owner: repository owner zemeng5208
- Verification environment: Windows pipe or the Linux peer, with two processes of the same OS user
- Acceptance criteria: a same-user process cannot gain another role's subscribe rights, or the owner accepts the limit for the release
- Close conditions: the test or the acceptance is recorded

<a id="oi-030"></a>

## OI-030

- Related requirement: [AS-008](STATUS.md)
- Source PR: plan 012. No SQLite crate was added. Replacing the segment format needs a separate dependency decision.
- Current state: recorded constraint
- Reason it is incomplete: this is the approved storage shape, not a missing test of SQLite
- Responsible PR: a future dependency plan, only if SQLite is proposed
- Handoff owner: repository owner zemeng5208
- Verification environment: not applicable while segments remain the store
- Acceptance criteria: the constraint stays until a dependency approval
- Close conditions: that approval, or the constraint remains the current design. It is not a defect.

<a id="oi-031"></a>

## OI-031

- Related requirement: [AS-009](STATUS.md) and [REQ-14.03](../roadmap/REQUIREMENTS.md)
- Source PR: plan 013. The portable vault does not use DPAPI, because that ciphertext binds to one Windows profile and blocks restore on another machine.
- Current state: recorded constraint
- Reason it is incomplete: DPAPI was not used, and it is not claimed as an accepted protection method
- Responsible PR: 038 and 042 still own REQ-14.03. A DPAPI design would be a new dependency and product decision.
- Handoff owner: repository owner zemeng5208
- Verification environment: not a DPAPI run
- Acceptance criteria: the portable vault stays free of DPAPI unless the owner approves a different, non-portable path
- Close conditions: the constraint remains, or a new approved design replaces it. REQ-14.03 is not accepted.

<a id="oi-032"></a>

## OI-032

- Related requirement: evidence for the plan 009 and plan 010 merge commits
- Source PR: merge `8fe3014846605a726249f64d58eaca9362eaacd4` and merge `145617f466fbb446085ec237a0593da50422e5d7`. When those records were written, the commit-status API returned no contexts. The workflow runs themselves succeeded. [Plan 009 main CI](https://github.com/zemeng5208/winstrata/actions/runs/46012223713). [Plan 010 main CI](https://github.com/zemeng5208/winstrata/actions/runs/46014395187).
- Current state: evidence gap
- Reason it is incomplete: no published status description was stored for those SHAs. This plan did not query the API again, so it does not invent a description.
- Responsible PR: the next records update
- Handoff owner: repository owner zemeng5208
- Verification environment: the GitHub commit-status API for those two SHAs
- Acceptance criteria: a later read records the description, or the owner accepts the workflow success without a stored description
- Close conditions: one of those two outcomes is written down. This is not a product defect.

<a id="oi-033"></a>

## OI-033

- Related requirement: [AG-12](../standards/GIT_AND_PR.md#ag-12). Closing another PR needs the owner's approval.
- Source PR: [GitHub PR #11](https://github.com/zemeng5208/winstrata/pull/11), head `4017bc7aa8219ea317aa27fb2b65c0d2cd8ef9a3`, still open
- Current state: open process item
- Reason it is incomplete: plan 051 carries the documentation intent. It does not close PR #11.
- Responsible PR: the owner closes PR #11
- Handoff owner: repository owner zemeng5208
- Verification environment: GitHub
- Acceptance criteria: PR #11 is closed by the owner after the documentation is on main, or the owner keeps it open for a stated reason
- Close conditions: the owner's action is recorded

<a id="oi-034"></a>

## OI-034

- Related requirement: plan 011 capture evidence
- Source PR: plan 011. The unprivileged live test records `capture_inaccessible` because `AF_PACKET` returns `EPERM`. A local root run of the committed test passed in 0.42 s. That root result is not CI. The Windows loopback exchange was not claimed as encryption evidence.
- Current state: known limitation
- Reason it is incomplete: CI did not have permission to capture, and the root run was local
- Responsible PR: 031
- Handoff owner: repository owner zemeng5208
- Verification environment: an authorized capture environment, named as CI or as an approved lab
- Acceptance criteria: the record states the privilege actually used and whether a TLS header was classified
- Close conditions: that record exists and does not cite the local root run as CI

<a id="oi-035"></a>

## OI-035

- Related requirement: the plan 004 toolchain note in the [platform report](../architecture/PLATFORM_FEASIBILITY.md)
- Source PR: plan 004. Known Vite advisory [GHSA-587h-7wmj-h5pm](https://github.com/advisories/GHSA-587h-7wmj-h5pm) affects upstream-locked development tooling.
- Current state: known limitation
- Reason it is incomplete: Plan 016 does not adopt Vite. The official shell is built with the plan 004 integrity-checked esbuild unpack from the approved sources' lockfiles. A production toolchain is still not selected, so the advisory has not been re-reviewed for one.
- Responsible PR: 016, before a production toolchain is selected
- Handoff owner: repository owner zemeng5208
- Verification environment: the advisory and the selected toolchain's lockfile
- Acceptance criteria: a fresh review is recorded before that toolchain is adopted
- Close conditions: the review is recorded. The trial dependency authorization does not close it.

<a id="e2e-01"></a>

## E2E-01

- Related requirement: [E2E-01](../roadmap/ACCEPTANCE.md#e2e-01)
- Source PR: the acceptance plan. No end-to-end run is recorded.
- Current state: not done
- Reason it is incomplete: plan 017 records the onboarding choices, scope, baseline, and mode markers on Linux. No real Windows launch was run. The scenario is not closed.
- Responsible PR: 017
- Handoff owner: repository owner zemeng5208
- Verification environment: a real Windows launch with a chosen data location
- Acceptance criteria: the scenario text in the acceptance plan
- Close conditions: a real run records the expected evidence. Not marked passed.

<a id="e2e-02"></a>

## E2E-02

- Related requirement: [E2E-02](../roadmap/ACCEPTANCE.md#e2e-02)
- Source PR: plans 009 and 010 record that E2E-02 was not run
- Current state: not done
- Reason it is incomplete: user-mode watches and process samples are not the GUI query over a chosen disk, other users, and the broader scenario
- Responsible PR: 021
- Handoff owner: repository owner zemeng5208
- Verification environment: Windows, with a chosen disk and test keys
- Acceptance criteria: the scenario text
- Close conditions: that run is recorded

<a id="e2e-03"></a>

## E2E-03

- Related requirement: [E2E-03](../roadmap/ACCEPTANCE.md#e2e-03)
- Source PR: plan 010
- Current state: not done
- Reason it is incomplete: software install and uninstall flows are later plans
- Responsible PR: 035
- Handoff owner: repository owner zemeng5208
- Verification environment: Windows with legitimate software to install and remove
- Acceptance criteria: the scenario text
- Close conditions: that run is recorded

<a id="e2e-04"></a>

## E2E-04

- Related requirement: [E2E-04](../roadmap/ACCEPTANCE.md#e2e-04)
- Source PR: plan 011
- Current state: not done
- Reason it is incomplete: topology, discovery, tagging, and export were not run
- Responsible PR: 031
- Handoff owner: repository owner zemeng5208
- Verification environment: an authorized Windows network
- Acceptance criteria: the scenario text
- Close conditions: that run is recorded

<a id="e2e-05"></a>

## E2E-05

- Related requirement: [E2E-05](../roadmap/ACCEPTANCE.md#e2e-05)
- Source PR: plan 012
- Current state: not done
- Reason it is incomplete: retention can be stored. Threshold notices, deletion, and index repair were not run.
- Responsible PR: 030
- Handoff owner: repository owner zemeng5208
- Verification environment: Windows with enough collected data to cross the thresholds, using lower thresholds first if needed
- Acceptance criteria: the scenario text
- Close conditions: that run is recorded

<a id="e2e-06"></a>

## E2E-06

- Related requirement: [E2E-06](../roadmap/ACCEPTANCE.md#e2e-06)
- Source PR: the acceptance plan. Plugins are not implemented.
- Current state: not done
- Reason it is incomplete: plans 014 through 017 and 043 have not started
- Responsible PR: 043
- Handoff owner: repository owner zemeng5208
- Verification environment: Windows with real plugins
- Acceptance criteria: the scenario text
- Close conditions: that run is recorded

<a id="e2e-07"></a>

## E2E-07

- Related requirement: [E2E-07](../roadmap/ACCEPTANCE.md#e2e-07)
- Source PR: the acceptance plan
- Current state: not done
- Reason it is incomplete: scripts and manual snapshot comparison are later plans
- Responsible PR: 027
- Handoff owner: repository owner zemeng5208
- Verification environment: the script workbench on Windows
- Acceptance criteria: the scenario text
- Close conditions: that run is recorded

<a id="e2e-08"></a>

## E2E-08

- Related requirement: [E2E-08](../roadmap/ACCEPTANCE.md#e2e-08)
- Source PR: plan 007
- Current state: not done
- Reason it is incomplete: CPU, GPU, memory, and budget behavior were not run as this scenario
- Responsible PR: 048
- Handoff owner: repository owner zemeng5208
- Verification environment: Windows under an authorized load
- Acceptance criteria: the scenario text
- Close conditions: that run is recorded

<a id="e2e-09"></a>

## E2E-09

- Related requirement: [E2E-09](../roadmap/ACCEPTANCE.md#e2e-09)
- Source PR: plan 012
- Current state: not done
- Reason it is incomplete: search, timeline, and export flows are later plans
- Responsible PR: 041
- Handoff owner: repository owner zemeng5208
- Verification environment: Windows with a real application across sources
- Acceptance criteria: the scenario text
- Close conditions: that run is recorded

<a id="e2e-10"></a>

## E2E-10

- Related requirement: [E2E-10](../roadmap/ACCEPTANCE.md#e2e-10)
- Source PR: plans 012 and 013
- Current state: not done
- Reason it is incomplete: Core encryption tests passed on Linux and on the plan 013 Windows Server 2022 review head. The scenario also needs lock and unlock while monitoring, rollback, plugin isolation, and import confinement. [OI-003](#oi-003), [OI-004](#oi-004), [OI-005](#oi-005), and [OI-008](#oi-008) are still open.
- Responsible PR: 042
- Handoff owner: repository owner zemeng5208
- Verification environment: Windows, with the vault locked during monitoring
- Acceptance criteria: the scenario text, and the four open items it depends on are closed or explicitly still excluded by the owner
- Close conditions: that run is recorded. The Core tests do not close it.

<a id="e2e-11"></a>

## E2E-11

- Related requirement: [E2E-11](../roadmap/ACCEPTANCE.md#e2e-11)
- Source PR: plan 004 measured three components. UI Moments is [OI-009](#oi-009).
- Current state: not done
- Reason it is incomplete: Linux tests persist a theme, reject driver commands, and exercise onboarding, pause, resume, and live/history through the loopback bridge. No real Windows WebView2 session was run. Dashboard drag remains plan 042.
- Responsible PR: 016
- Handoff owner: repository owner zemeng5208
- Verification environment: real Windows WebView2
- Acceptance criteria: the scenario text, including three themes
- Close conditions: that run is recorded

<a id="e2e-12"></a>

## E2E-12

- Related requirement: [E2E-12](../roadmap/ACCEPTANCE.md#e2e-12)
- Source PR: the acceptance plan
- Current state: not done
- Reason it is incomplete: local MCP product flows are later plans
- Responsible PR: 045
- Handoff owner: repository owner zemeng5208
- Verification environment: a real local agent on the same machine
- Acceptance criteria: the scenario text
- Close conditions: that run is recorded

<a id="e2e-13"></a>

## E2E-13

- Related requirement: [E2E-13](../roadmap/ACCEPTANCE.md#e2e-13)
- Source PR: the acceptance plan
- Current state: not done
- Reason it is incomplete: rescue is plan 047 and has not started
- Responsible PR: 047
- Handoff owner: repository owner zemeng5208
- Verification environment: an isolated Windows host with harmless targets
- Acceptance criteria: the scenario text
- Close conditions: that run is recorded

<a id="e2e-14"></a>

## E2E-14

- Related requirement: [E2E-14](../roadmap/ACCEPTANCE.md#e2e-14)
- Source PR: plan 008
- Current state: not done
- Reason it is incomplete: update, service, driver, security, and certificate observation is later
- Responsible PR: 036
- Handoff owner: repository owner zemeng5208
- Verification environment: an authorized Windows host
- Acceptance criteria: the scenario text
- Close conditions: that run is recorded

<a id="e2e-15"></a>

## E2E-15

- Related requirement: [E2E-15](../roadmap/ACCEPTANCE.md#e2e-15)
- Source PR: plans 007 and 008
- Current state: not done
- Reason it is incomplete: sustained, burst, disk-full, and unexpected-exit scenarios were not run
- Responsible PR: 048
- Handoff owner: repository owner zemeng5208
- Verification environment: an authorized Windows host that can be driven into those conditions
- Acceptance criteria: the scenario text
- Close conditions: that run is recorded

<a id="e2e-16"></a>

## E2E-16

- Related requirement: [E2E-16](../roadmap/ACCEPTANCE.md#e2e-16)
- Source PR: the acceptance plan. Plan 000 checked the then-current 61 rules and 51 plans. That historical check is not this scenario.
- Current state: not done
- Reason it is incomplete: final release reconciliation has not been run. Main `a2471d5fc13f0a307ee39d2629c454aea4537809` failed Windows workspace checks. Plan 052 later passed them on `004974215e239f96160b47d3eef40a84c316e802`. See [ERR-005](modules/core.md#err-005).
- Responsible PR: 050
- Handoff owner: repository owner zemeng5208
- Verification environment: the exact merge candidate and actual main
- Acceptance criteria: the scenario text, including 64 rules after AG-64 is in effect
- Close conditions: that run is recorded on a main commit whose required checks are green

<a id="oi-036"></a>

## OI-036

- Related requirement: [REQ-19.01](../roadmap/REQUIREMENTS.md) and [REQ-19.15](../roadmap/REQUIREMENTS.md)
- Source PR: unmerged GitHub PR #21, `pr/016-desktop-gui`, head `64bd627c362ad025869905441bee7802dda19ac1`. Main `0accacb` has no GUI file.
- Current state: register matches this commit; E2E-19 still open
- Reason it is incomplete: This commit's source register matches the vendored files, including the UI Moments originals. Closed PR #21's `packages/ui/static/shell.js` and `packages/ui/static/themes.css` are not in this commit. WebView2 measurement remains [OI-009](#oi-009). E2E-19 has no implementation plan ID and is not plan 016 acceptance.
- Responsible PR: plan 016, on its existing row. Plan 053 does not add itself to that dependency cell and does not edit the branch.
- Handoff owner: repository owner zemeng5208
- Verification environment: the plan 016 change that merges official GUI files, compared with the upstream revisions
- Acceptance criteria: [E2E-19](../roadmap/ACCEPTANCE.md#e2e-19) for files in that commit. The host-authored layout and paint are either replaced by the approved sources or recorded as an owner-approved gap.
- Close conditions: the source register rows match the files in the same commit. Not closed by this standards change.

<a id="oi-037"></a>

## OI-037

- Related requirement: [REQ-19.06](../roadmap/REQUIREMENTS.md) through [REQ-19.14](../roadmap/REQUIREMENTS.md)
- Source PR: the 2026-11-28 owner decision, recorded as proposed [AG-64](../../AGENTS.md#ag-64)
- Current state: unimplemented
- Reason it is incomplete: main has no GUI. Full replacement, the frontend SDK, decoupling under a second GUI, and the trusted-host prompts were not implemented. Plan 016's existing official-GUI acceptance is unchanged.
- Responsible PR: no roadmap row. The owner assigns the follow-up. It is not plan 015 or plan 016.
- Handoff owner: repository owner zemeng5208
- Verification environment: real Windows, with the official GUI installed and one third-party GUI plugin
- Acceptance criteria: [E2E-17](../roadmap/ACCEPTANCE.md#e2e-17) and [E2E-18](../roadmap/ACCEPTANCE.md#e2e-18)
- Close conditions: those runs are recorded after the owner has approved the technical approach and assigned a roadmap row. Not closed by a static page.

<a id="oi-038"></a>

## OI-038

- Related requirement: [REQ-19.16](../roadmap/REQUIREMENTS.md)
- Source PR: the 2026-11-28 owner decision
- Current state: pending decision
- Reason it is incomplete: the owner required the technical approach to be researched and verified later. This change does not select Tauri, React, Vite, or any other stack.
- Responsible PR: no implementation plan ID. Plan 053 is the standards row immediately after 014. An implementation plan, if the owner adds one, is not another ID in this change.
- Handoff owner: repository owner zemeng5208
- Verification environment: the proposal's own measured evidence, recorded before implementation
- Acceptance criteria: [the proposal section](../roadmap/ACCEPTANCE.md#gui-proposal)
- Close conditions: the owner approves a verified proposal, or records that the capability is withdrawn. Not closed by naming a candidate.

<a id="e2e-17"></a>

## E2E-17

- Related requirement: [E2E-17](../roadmap/ACCEPTANCE.md#e2e-17)
- Source PR: the 2026-11-28 owner decision
- Current state: not done
- Reason it is incomplete: no third-party GUI plugin exists on main
- Responsible PR: no roadmap row
- Handoff owner: repository owner zemeng5208
- Verification environment: real Windows with one third-party GUI plugin and the official GUI still installed
- Acceptance criteria: the scenario text
- Close conditions: that run is recorded. Plan 016's E2E-11 does not close it.

<a id="e2e-18"></a>

## E2E-18

- Related requirement: [E2E-18](../roadmap/ACCEPTANCE.md#e2e-18)
- Source PR: the 2026-11-28 owner decision
- Current state: not done
- Reason it is incomplete: the trusted-host prompts for a third-party GUI were not implemented
- Responsible PR: no roadmap row
- Handoff owner: repository owner zemeng5208
- Verification environment: real Windows, including an isolated host for rescue and deletion attempts
- Acceptance criteria: the scenario text
- Close conditions: that run is recorded

<a id="e2e-19"></a>

## E2E-19

- Related requirement: [E2E-19](../roadmap/ACCEPTANCE.md#e2e-19)
- Source PR: the source register created with AG-64. The product table is empty at `0accacb`.
- Current state: not done
- Reason it is incomplete: no official GUI file is in this commit
- Responsible PR: the change that adds the files. No new roadmap row.
- Handoff owner: repository owner zemeng5208
- Verification environment: the commit that contains the official GUI or official plugin presentation
- Acceptance criteria: the scenario text
- Close conditions: the register and the files match. Not closed by the empty table.
