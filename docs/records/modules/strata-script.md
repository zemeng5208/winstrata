# StrataScript error register

[简体中文](../../zh-CN/records/modules/strata-script.md) · [Global register](../ERRORS.md) · [Script plugin](../../architecture/SCRIPT.md)

Authoritative error records for `plugins/strata-script`.

[ERR-022](#err-022) is resolved and stays in this history. The Windows AppContainer gap in [Containment](../../architecture/SCRIPT.md#containment) is [OI-045](../OPEN_ITEMS.md#oi-045), not an error recorded here. The Windows scripts page is [OI-044](../OPEN_ITEMS.md#oi-044). Plan 022 owns [ERR-020](strata-services.md#err-020).

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
