# Core error register

[简体中文](../../zh-CN/records/modules/core.md) · [Global register](../ERRORS.md) · [Open items](../OPEN_ITEMS.md)

Authoritative error records for `core`. [AG-63](../../../AGENTS.md#ag-63) requires these fields. STATUS.md is not a substitute.

<a id="err-016"></a>

## ERR-016

- Error ID: ERR-016
- Discovery time: 2026-11-27, Linux workspace job on `58452bd` during plan 007
- Related PR: plan 007, before merge
- Environment: GitHub Actions Linux workspace
- Reproduction method: `cargo test` reached `unsubscribe_and_shutdown_release_queues_without_keeping_threads`
- Evidence: [Linux failure](https://github.com/zemeng5208/winstrata/actions/runs/45982561210/job/140450010980). `live_threads` was already 0, and one thread id still matched the scheduler name immediately after join. Windows workspace checks on that commit succeeded.
- Root cause: the check treated a thread id that still carried the scheduler name immediately after join as a live thread. The later fix records each thread's start time and waits up to two seconds for the kernel to drop the task entry. A deeper scheduler leak was not shown.
- Impact: Linux workspace checks failed. The merge-policy evaluation on that commit succeeded.
- Fix plan: implemented in plan 007 before merge. This record does not change that code.
- Retest result: eight consecutive local `cargo test --locked -p winstrata-core --lib -- --test-threads=16` runs passed, including that test. GitHub Workspace, Linux, and Windows checks succeeded on `0dceae00c8693cd84c6c061538206233622a27da`. Main CI succeeded on the plan 007 merge `3e0097a44f54d552662b39ab3621002ebce59505`.
- Current state: resolved

<a id="err-009"></a>

## ERR-009

- Error ID: ERR-009
- Discovery time: 2026-11-27, Windows workspace job on `53408d7e2ba450b5cacc15923c513f9fc9e1881b` during plan 012
- Related PR: plan 012, before merge
- Environment: GitHub Actions `windows-2022`
- Reproduction method: `cargo test --locked --workspace` entered `writes_query_segments_index_and_capacity` and `Hybrid::create`
- Evidence: Windows error 1305, “The revision level is unknown.” [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/45998553679/job/140504032301). The same Rust code had passed on `dfe59001d1647d8093fe8a90881efdc9a397f045`. Other hybrid creates in the failing process passed.
- Root cause: the recorded cause is a security-descriptor revision that could not be read. It is not a failed DACL comparison. Why that read failed once is otherwise unknown.
- Impact: Windows workspace checks failed, so Workspace checks failed
- Fix plan: retry error 1305 at most four times while creating a private directory and while rereading its descriptor. Owner and ACE rejection checks stay. Implemented in plan 012 before merge.
- Retest result: `5243b91d50948f89d92635adb7a02f169f89c54f` passed all 33 Windows Core tests, including the previously failing test
- Current state: resolved

<a id="err-010"></a>

## ERR-010

- Error ID: ERR-010
- Discovery time: 2026-11-27, Windows workspace job on `b6481384b0796c1dccd526202613036cba346c69` during plan 012
- Related PR: plan 012, before merge
- Environment: GitHub Actions `windows-2022`
- Reproduction method: `cargo test --locked --workspace` entered `hybrid::tests::measured_event_batch_round_trips` and `Hybrid::create`
- Evidence: Windows error 1336, “The access control list (ACL) structure is invalid.” [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46021115653/job/140580685441). Other hybrid creates in that process passed. Kernel and service tests did not run because Core stopped.
- Root cause: the security-descriptor read returned 1336 before the owner and ACE comparison. A mismatched owner or ACE still returns permission denied and is not retried. Why the read failed once is otherwise unknown.
- Impact: Windows workspace checks failed. That head was not a review head.
- Fix plan: the same read retries errors 1305 and 1336 at most four times and still returns the error if it persists. Implemented in plan 012 before merge.
- Retest result: `31f15a72cc2ff4d63136caa8517dd64a448a0001` passed, including `measured_event_batch_round_trips`
- Current state: resolved

<a id="err-011"></a>

## ERR-011

- Error ID: ERR-011
- Discovery time: 2026-11-27, Windows jobs on `83cd44377fd4588c7706a6f227ca6e819a7e177e` and `798b91ff5a394d6fec2820fe0142ecac873fcb35` during plan 009
- Related PR: plan 009, before merge
- Environment: GitHub Actions `windows-2022`
- Reproduction method: `cargo test --locked --workspace` ran the file and registry watch tests
- Evidence: on `83cd443` Core finished 34 passed and 4 failed. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46007410073/job/140533972873). On `798b91f` two file tests saw only `gap:SourceFailure`. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46008317408/job/140537032504).
- Root cause: the first failure used a stack address for the `ReadDirectoryChangesW` byte count and a JSON subject check that could not match escaped backslashes. The second failure stored the kernel completion in an `OVERLAPPED` value that the function then moved. The recorded fix keeps the overlapped structure, byte count, and buffer at the address given to the first call.
- Impact: Windows workspace checks failed on both heads
- Fix plan: implemented in plan 009 before merge
- Retest result: `565599ed4f6125fdfa737d4e83087027211879c2` passed the three Windows file and registry tests
- Current state: resolved

<a id="err-012"></a>

## ERR-012

- Error ID: ERR-012
- Discovery time: 2026-11-27, Windows job on `62b7bc1dde69cd793373e48abcdd8891e2a5c022` during plan 013
- Related PR: GitHub PR #16, plan 013, before the review head
- Environment: GitHub Actions `windows-2022`, version 10.0.20348
- Reproduction method: `cargo test --locked --workspace` ran `backup_and_restore_keep_ciphertext_integrity`
- Evidence: 58 Core tests passed and that test failed. The tampered package was written with the default ACL, so private-file verification returned `PermissionDenied` before the checksum could return `InvalidData`. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46026990013/job/140600824049).
- Root cause: the tampered package did not use the private-file ACL used by a real backup
- Impact: Windows workspace checks failed, so Workspace checks failed
- Fix plan: create the tampered package with the same private-file call and still require `InvalidData`. Implemented before merge.
- Retest result: review head `36b5cd688afa2f9690f4a634d0a4c57ec6eba7cd` passed 59 Core tests, including that test. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46027463216/job/140602443474).
- Current state: resolved

<a id="err-005"></a>
<a id="err-013"></a>

## ERR-005

- Error ID: ERR-005. ERR-013 is the same defect, not a second error.
- Discovery time: 2026-11-27T15:49:51Z
- Related PR: GitHub PR #16 merge, plan 013, main `a2471d5fc13f0a307ee39d2629c454aea4537809`
- Environment: GitHub Actions `windows-2022`. The runner label is Windows Server 2022. It is not a Windows 10 or Windows 11 client.
- Reproduction method: WinStrata CI on that main commit runs `cargo test --locked --workspace` in the step “Test the workspace on Windows”. The suite panicked in `process::tests::real_process_tree_pid_lifecycle_and_dedup` at `core/src/process.rs:905`, assertion `has_event(&third, PROCESS_EXITED, pid, start)`.
- Evidence: [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46028223956/job/140605027762) completed at 2026-11-27T15:49:51Z with conclusion failure. [Linux job](https://github.com/zemeng5208/winstrata/actions/runs/46028223956/job/140605027316) succeeded at 2026-11-27T15:49:30Z. [Workspace job](https://github.com/zemeng5208/winstrata/actions/runs/46028223956/job/140605466118) failed because Windows failed. The merge parents are `be5eb0d699ad0a7e6aab46df6f6bbbf3d912f8af` and `31f22d3f0f9d5ef0e9120f9ba6b75af868ce32a7`. That second parent has the same tree as main. Its only difference from review head `36b5cd688afa2f9690f4a634d0a4c57ec6eba7cd` is the STATUS records, so the Rust test code matches the review head. The review-head Windows job passed 59 Core tests, so this assertion did not fail there. [Review-head Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46027463216/job/140602443474). The same tree also passed on `31f22d3`. [Later review-head Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46027962309/job/140604132924). The failure log does not include the sample.
- Root cause: unknown. The assertion failed. The same test code had passed on the review head minutes earlier, so the failure may be intermittent. That possibility is not a root cause and does not close the error. A truncated sample, or a PID that remains listed without a positive start time, both suppress `process.exited`. Which of those two occurred on this run was not captured.
- Impact: confirmed CI failure on main `a2471d5`. Workspace checks failed. [REQ-06.02](../../roadmap/REQUIREMENTS.md) was not accepted for that run. Plan 052 is the designated product fix. Plan 051 does not change the test. The plan 007 thread-release failure is [ERR-016](#err-016).
- Fix plan: plan 052 keeps the exit assertion and waits for a complete sample that omits the instance or replaces its start time. It does not treat a missing event as success. The owner merged that plan as `004974215e239f96160b47d3eef40a84c316e802`. Plan 051 does not change `core/src/process.rs` or the test.
- Retest result: Windows workspace checks on merge `004974215e239f96160b47d3eef40a84c316e802` passed. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46036724064/job/140633882007). Linux and Workspace checks on that commit also passed. The root cause of the earlier failure remains unknown.
- Current state: resolved

<a id="err-014"></a>

## ERR-014

- Error ID: ERR-014
- Discovery time: 2026-11-27, during plan 006, before candidate `b3f56ab`. The failing commit SHA is unknown.
- Related PR: GitHub PR #7, plan 006
- Environment: Windows, during config replacement. The job URL is unknown.
- Reproduction method: replace configuration while long-lived pins were held on shared ancestors. The exact command of the failing candidate is unknown.
- Evidence: the plan 006 record says earlier candidates hit a Windows sharing violation during config replacement, and long-lived pins on shared ancestors interfered with sibling renames. No failing SHA or job URL was stored.
- Root cause: the session held pins that blocked the replacement and sibling renames. The session now holds only its locator lock file between operations.
- Impact: those earlier Windows candidates failed. The later recorded candidate passed.
- Fix plan: implemented in plan 006 before merge
- Retest result: candidate `b3f56ab` passed 20 Core tests on Windows Server 2022. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/45976058358/job/140427947209). A native NTFS regression confirmed that a private directory stays pinned while an ordinary sibling can be renamed.
- Current state: resolved

<a id="err-018"></a>

## ERR-018

- Error ID: ERR-018
- Discovery time: 2026-11-27, Windows workspace job on `df34c741e80d02e711dc97b8b8eeec34eae05f24` during plan 017
- Related PR: GitHub PR #28, plan 017, before merge
- Environment: GitHub Actions `windows-2022`. The runner label is Windows Server 2022. It is not a Windows 10 or Windows 11 client.
- Reproduction method: `cargo test --locked --workspace` reached `scope::tests::foreign_profiles_and_unselected_types_stay_out`, `scope::tests::registry_process_and_interface_choices_are_explicit`, and `onboarding::tests::baseline_scope_pause_history_and_loss_stay_distinct`
- Evidence: [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46058310468/job/140706127654). `admits_file` and `admits_process` rejected a path under `std::env::temp_dir()`. `build_baseline` then returned `a full snapshot needs at least one entry`. Linux workspace checks on the same commit passed. [Linux job](https://github.com/zemeng5208/winstrata/actions/runs/46058310468/job/140706126963).
- Root cause: the runner temporary directory is inside the current user's profile, and Windows spelled that profile with an 8.3 short name that did not match the owner parsed from `USERPROFILE`. The scope filter treated the current user's own directory as another user's private profile.
- Impact: Windows workspace checks failed, so Workspace checks failed. The selected baseline could not be stored for a directory under that profile.
- Fix plan: compare the current profile's long name and 8.3 short name, and expand an existing ancestor of the candidate path. A different user's private profile stays excluded. A selected scope with no matching files stores the baseline object and does not write an empty snapshot. The tests are unchanged.
- Retest result: `f4c95f2d0e0077d765ede17278348a679c75e165` passed Windows workspace checks. The pull request job checked out merge `5d8e81711740c84fab7d32bdc5270a5cdd5e89aa`. Its parents are `ad10523d81870d106eeef553a426beeb250e94ff` and `f4c95f2d0e0077d765ede17278348a679c75e165`, and its tree `89b473da381c91a748bfa01e025b394650d87326` equals that head. Windows Server 2022 ran `cargo test --locked --workspace` and passed 66 Core tests, including the three tests named above, 24 kernel tests, 14 protocol library tests, 2 protocol integration tests, 1 live system-event test, 1 Windows language test, 5 runtime lifecycle tests, 7 service tests, and 5 desktop tests. [Windows](https://github.com/zemeng5208/winstrata/actions/runs/46059119581/job/140708786928). Linux workspace checks and Workspace checks on that head also passed. This job is Windows Server 2022. It is not a Windows 10/11 client or a WebView2 session.
- Current state: resolved

<a id="err-023"></a>

## ERR-023

- Error ID: ERR-023
- Numbering: plan 022 owns ERR-020 and ERR-021. The open plan 027 branch owns ERR-022. This pulse defect is ERR-023.
- Discovery time: 2026-11-27T21:04:07Z
- Related PR: GitHub PR #33, plan 024, head `a2606194d44534b1e9f0791711e5fc93c6c1704f`
- Environment: GitHub Actions `windows-2022`, Windows Server 2022
- Reproduction method: `cargo test --locked --workspace` reached `counters_history_and_unsupported_readings_survive_pause_and_disable`
- Evidence: the sample document had `store` equal to `failed` after CPU and memory were measured. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46067628416/job/140736681773). Linux workspace checks on that head passed. The published `winstrata/merge-policy` status failed with “Plan 022 must merge before plan 024”. That order result is expected and is not this defect.
- Root cause: `commit_plugin_metrics` passed the canonical state path to `Anchor::open`. On Windows that path begins with `\\?\`, and the anchor accepts only a drive path, so every metric commit failed.
- Impact: Windows history stayed in the private files, and the plan 012 series was not written. Windows workspace checks failed, so Workspace checks failed.
- Fix plan: drop a canonical `\\?\` drive prefix before opening the anchor. The directory-shape check stays. The pulse observe test stays.
- Retest result: on `c827132aa9d072a0239594d12729e53c2ec81eff`, Windows Server 2022 `cargo test --locked --workspace` passed, including `counters_history_and_unsupported_readings_survive_pause_and_disable`. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46068088739/job/140738171555). Linux workspace checks and Workspace checks on that head also passed. [Linux](https://github.com/zemeng5208/winstrata/actions/runs/46068088739/job/140738171180) · [Workspace](https://github.com/zemeng5208/winstrata/actions/runs/46068088739/job/140738791133). The published `winstrata/merge-policy` status on that head failed with “Plan 022 must merge before plan 024”. [Policy run](https://github.com/zemeng5208/winstrata/actions/runs/46068086984). That order result is expected and is not this defect. This later record writes that retest down, so its own four gates are not those results.
- Current state: resolved

<a id="err-026"></a>

## ERR-026

- Error ID: ERR-026
- Discovery time: 2026-11-28, during local Windows 11 verification after main `4f257ae`
- Related PR: plan 054, branch `pr/054-windows11-remediation` (source behavior from plan 015)
- Environment: Windows 11 Home Chinese x64 (10.0.26200, build 26200, ordinary unelevated user), Python 3.12 with project venv launcher on `PATH`, Node.js 24.22.3
- Reproduction method: `. .\.winstrata\env.ps1; cargo test --locked -p winstrata-runtime --test languages_windows`
- Evidence: `windows_language_plugins_enforce_the_container_boundary` failed during Windows AppContainer startup first with `isolation_unavailable: sandbox grant exceeded the file limit` when recursively walking host runtime trees (>20,000 entries under host `site-packages`), and on Python launch after narrowing runtime grants failed with `No pyvenv.cfg file` inside `stderr.log`. Dropping an active grant also left explicit AppContainer SID ACEs on shared runtime paths until revocation was added.
- Root cause: (1) `grant_tree` recursively visited every descendant of the Python/Node installation prefix, exceeding the 20,000-file grant cap and exposing host `site-packages`; (2) passing `.winstrata/venv/Scripts/python.exe` directly on `cfg!(windows)` invoked the venv redirector binary inside a 1-process Job Object and AppContainer without access to the outer `.winstrata/venv/pyvenv.cfg`; (3) `Profile`/`Guard` did not revoke granted ACEs upon drop, and protected DACLs were touched on non-protected inherited descendants.
- Impact: Python language plugins could not start inside the Windows AppContainer on a Windows 11 host with populated runtime trees or a project venv on `PATH`, and test cleanup left AppContainer SID ACEs on the runtime directory.
- Fix plan: in `core/src/plugins/sandbox.rs` and `core/src/plugins/language.rs`, resolve `sys._base_executable` on `cfg!(windows)` in `Toolchain::bind`, replace recursive runtime tree walking with non-inheritable top-level/stdlib-only prefix grants (`grant_python_prefix` skipping `site-packages` and `__pycache__`, `grant_node_prefix` skipping subdirectories such as `node_modules`), only grant `SE_DACL_PROTECTED` descendants inside `walk_grant`, and revoke all recorded ACEs via `revoke_all` on drop. Implemented on branch `pr/054-windows11-remediation`.
- Retest result: `cargo test --locked -p winstrata-runtime --test languages_windows` passed 2/2 tests with `RUSTFLAGS="-Dwarnings"` (`windows_language_plugins_enforce_the_container_boundary` and `declared_windows_node_and_python_match_project_pins`), verifying local/catalog module load, denied host file read, denied TCP socket, denied HKCU registry access, denied `cmd.exe` spawn, private file preservation, and clean `tasklist` exit plus ACE revocation (`icacls` confirmed no `S-1-15-2-*` residue).
- Current state: resolved on branch `pr/054-windows11-remediation`

<a id="err-028"></a>

## ERR-028

- Error ID: ERR-028
- Discovery time: 2026-11-28, during plan 054 expansion of `languages_windows.rs`
- Related PR: plan 054, branch `pr/054-windows11-remediation` (source behavior from plan 015)
- Environment: Windows 11 Home Chinese x64 (10.0.26200, build 26200, ordinary unelevated user) under desktop `WinSta0\exebox-*` (`(A;;CCDTCRRC;;;AC)`) with zero-capability AppContainer and Node.js 24.22.3 (`win-x64`)
- Reproduction method: run JavaScript/TypeScript plugin (`node.exe` 24.22.3) via `spawn_windows` inside a zero-capability AppContainer (`S-1-15-2-*`), or launch an isolated `node.exe -e "process.exit(42)"` probe in the same container
- Evidence: `host.enable("fixture.jswin")` failed on first IPC frame read with `unavailable: failed to fill whole buffer` and 0-byte `stderr.log`. Standalone PowerShell/C AppContainer probe returned exit code `0xC0000142` (`STATUS_DLL_INIT_FAILED`) for `node.exe`, `user32`, `ole32`, and `shell32` probes, while `kernel32`, `crypt32`, `winmm`, and `dbghelp` returned `42`. `dumpbin /dependents` confirmed `node.exe` statically imports `USER32.dll`, `SHELL32.dll`, and `ole32.dll`, whereas `python.exe` / `python312.dll` import only `KERNEL32.dll`, `ADVAPI32.dll`, `WS2_32.dll`, `bcrypt.dll`, and `VERSION.dll`. Thread desktop SDDL showed `ALL APPLICATION PACKAGES` (`AC`) granted `CCDTCRRC` only.
- Root cause: `node.exe` statically links `USER32.dll` / `SHELL32.dll` / `ole32.dll`, whose DLL initialization fails with `STATUS_DLL_INIT_FAILED` (`0xC0000142`) inside a zero-capability AppContainer whenever the inherited Window Station / Desktop DACL denies desktop access required by `USER32.dll` (observed on the restricted execution desktop and applicable to non-interactive Session 0 service desktops).
- Impact: JavaScript and TypeScript language plugins fail to initialize on Windows hosts/sessions whose inherited desktop DACL does not grant `AC` full user32 desktop initialization rights. Python language plugins are unaffected and pass.
- Fix plan: unassigned follow-up (requires sanctioned Windows Desktop/Window Station isolation or Node/JS runtime boundary design without weakening containment or tampering with host desktop ACLs). Tracked by [OI-062](../OPEN_ITEMS.md#oi-062).
- Retest result: untested pending approved design
- Current state: open
