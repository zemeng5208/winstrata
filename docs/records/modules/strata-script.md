# StrataScript error register

[简体中文](../../zh-CN/records/modules/strata-script.md) · [Global register](../ERRORS.md) · [Script plugin](../../architecture/SCRIPT.md)

Authoritative error records for `plugins/strata-script`.

[ERR-022](#err-022) is resolved and stays in this history. [ERR-024](#err-024) is open. The Windows AppContainer gap in [Containment](../../architecture/SCRIPT.md#containment) is [OI-045](../OPEN_ITEMS.md#oi-045), not an error recorded here. The Windows scripts page is [OI-044](../OPEN_ITEMS.md#oi-044). Plan 022 owns [ERR-020](strata-services.md#err-020).

<a id="err-022"></a>

## ERR-022

- Error ID: ERR-022
- Discovery time: 2026-11-27T21:13:42Z
- Related PR: GitHub PR #34, plan 027, head `e08cf7ee2dfe42303276a0426d7cec46a6f02867`
- Environment: GitHub Actions `windows-2022`, Windows Server 2022
- Reproduction method: `cargo test --locked --workspace` reached `editor_languages_debug_triggers_logs_permissions_and_empty_presets`
- Evidence: the test panicked at `plugins/strata-script/tests/workbench.rs:237` with `isolation shim is not beside the script program`. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46068679662/job/140740093587). Linux workspace checks on that head passed. The published `winstrata/merge-policy` status failed with “Plan 022 must merge before plan 027”. That order result is expected and is not this defect.
- Root cause: the test required `winstrata_shim.dll` beside the script program. `cargo test` does not build the cdylib-only shim package. The Windows job does not run the workspace build that places `libwinstrata_shim.so` beside the program on Linux. Windows containment does not load this file.
- Impact: Windows workspace checks failed before a script ran. Workspace checks then failed because Windows failed.
- Fix plan: require the shim file only on Unix, where Landlock uses it. Keep the Windows workbench assertions. Do not mark [OI-045](../OPEN_ITEMS.md#oi-045) passed.
- Retest result: on head `a66da7b749ef5657d55f72cccd2d699783dd4a6f`, Windows Server 2022 `cargo test --locked --workspace` passed. `editor_languages_debug_triggers_logs_permissions_and_empty_presets` passed. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46069484949/job/140742722189). That run does not close [OI-044](../OPEN_ITEMS.md#oi-044) or [OI-045](../OPEN_ITEMS.md#oi-045).
- Current state: resolved

<a id="err-024"></a>

## ERR-024

- Error ID: ERR-024
- Discovery time: 2026-11-27T23:34:37Z
- Related PR: GitHub PR #38, plan 029, head `e3dcdd2a89df501b885f3941f4d5312c035126d8`
- Environment: GitHub Actions `windows-2022`, Windows Server 2022. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46083084398/job/140786337911)
- Reproduction method: `cargo test --locked --workspace` reached `editor_languages_debug_triggers_logs_permissions_and_empty_presets`
- Evidence: the test panicked at `plugins/strata-script/tests/workbench.rs:379`. `debug-start` returned `{"code":"faulted","ok":false,"presets":0}`. The job then terminated an orphan `python` process. Linux workspace checks on that head passed. [Linux job](https://github.com/zemeng5208/winstrata/actions/runs/46083084398/job/140786338253). Workspace checks failed because Windows failed. [Workspace job](https://github.com/zemeng5208/winstrata/actions/runs/46083084398/job/140786937292). The published `winstrata/merge-policy` status failed with “PR is in draft”. That draft result is expected and is not this defect.
- Root cause: `wait_pause` treated `debug/pause.json` as complete as soon as the file existed. The debugger creates that file before the JSON line is finished. A partial read became `faulted`, and the child was dropped without being killed.
- Impact: Windows script debugging returned `faulted` instead of `paused`. Windows workspace checks failed. Workspace checks failed with them. This blocks plan 029 until the Windows retest passes.
- Fix plan: keep reading until the file contains a line number or the debug wait ends. Kill the child when that wait ends in `faulted`. The Linux unit test `a_partial_pause_file_is_read_after_it_is_complete` covers a file that starts empty.
- Retest result: unknown on Windows. The Linux unit test passed on this authoring host, and `editor_languages_debug_triggers_logs_permissions_and_empty_presets` passed again on Linux. Those Linux runs do not close this record. [OI-044](../OPEN_ITEMS.md#oi-044) and [OI-045](../OPEN_ITEMS.md#oi-045) stay open.
- Current state: open
