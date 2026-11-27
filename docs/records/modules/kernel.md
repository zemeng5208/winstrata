# Kernel error register

[简体中文](../../zh-CN/records/modules/kernel.md) · [Global register](../ERRORS.md)

Authoritative error records for `kernel`.

<a id="err-003"></a>

## ERR-003

- Error ID: ERR-003
- Discovery time: 2026-11-27, during plan 003. A clock time was not recorded.
- Related PR: GitHub PR #4, plan 003
- Environment: Linux x86_64 link of the kernel PE32+ probe against the pinned WDK import library
- Reproduction method: `cargo dev cross-probe` linked the kernel image
- Evidence: the first link failed because the selected x64 WDK import archive did not export `KeQueryInterruptTime`. The archive was inspected. The probe switched to the documented export `KeQueryTimeIncrement`. Both probes and PE inspection were rerun. No Windows driver was executed.
- Root cause: the selected import was not an export of that archive
- Impact: the first Linux driver link failed
- Fix plan: implemented in plan 003 before merge
- Retest result: the recorded plan 003 probes linked and were byte-identical across two runs
- Current state: resolved

<a id="err-007"></a>

## ERR-007

- Error ID: ERR-007
- Discovery time: 2026-11-27, Windows workspace job on `fcb7e3d61881a87e05d92f76846df86d6fdd7778` during plan 011
- Related PR: GitHub PR #14, plan 011, before the later green head
- Environment: GitHub Actions `windows-2022`
- Reproduction method: `cargo test --locked --workspace`. Core reached the live exchange and `Observer::collect` returned Winsock error 10022 (`WSAEINVAL`).
- Evidence: [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46007292420/job/140533560835). An unbound `SOCK_RAW` returns that error from `recv`, and the error discarded the connection tables before the test could read them. Linux on that commit passed.
- Root cause: the raw socket was not bound, so `recv` returned `WSAEINVAL` and the collector dropped the tables
- Impact: Windows workspace checks failed, so Workspace checks failed
- Fix plan: bind the raw socket to one non-loopback IPv4 address and keep a capture error on the gap list. Implemented in plan 011 before merge.
- Retest result: `ab01883653187bbebd5e9197a80a92b784d89e36` passed both live network tests on Windows Server 2022
- Current state: resolved

<a id="err-008"></a>

## ERR-008

- Error ID: ERR-008
- Discovery time: 2026-11-27. First recorded Windows failure on `34385e9ad0c097d236dc2fce3a2bb675bd4042cb` during plan 011. The same assertion failed again on plan 012 head `3a4136ece12b511b225812cad51b42d64ffd4649` after main `9f70e62` was merged.
- Related PR: plans 011 and 012, before their review heads
- Environment: GitHub Actions `windows-2022`
- Reproduction method: `service_control_manager_refuses_to_start_a_missing_kernel_image_and_the_entry_is_removed` creates `WinStrata008MissingImage`, `StartService` fails, `DeleteService` succeeds, and the next `OpenServiceW` still opens the probe
- Evidence: plan 011 panic “probe service still exists”. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46015372432/job/140561018902). Plan 012 failed at `kernel/src/scm.rs:234` with the same message. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46004612235/job/140524531045). `kernel/src/scm.rs` was unchanged from a main commit where the test had passed.
- Root cause: `DeleteService` had returned success and the handle was closed, but `OpenServiceW` still found the probe immediately. The Service Control Manager delay beyond that recorded race is unknown.
- Impact: Windows workspace checks failed on both heads
- Fix plan: wait up to two seconds for `ERROR_SERVICE_DOES_NOT_EXIST`, close any handle the check opens, and still fail if the probe remains. The test is not skipped. Implemented before plans 011 and 012 merged.
- Retest result: `e0abe645789b1f32173612d22ce4459d42191dee` passed the test. `804576c7e64fe13685982a59f59e1ad8cb8257cf` passed it with 9 kernel tests and 0 failures.
- Current state: resolved
