# StrataScript error register

[简体中文](../../zh-CN/records/modules/strata-script.md) · [Global register](../ERRORS.md) · [Script plugin](../../architecture/SCRIPT.md)

Authoritative error records for `plugins/strata-script`.

[ERR-020](#err-020) is open. The Windows AppContainer gap in [Containment](../../architecture/SCRIPT.md#containment) is [OI-041](../OPEN_ITEMS.md#oi-041), not an error recorded here. The Windows workbench session is [OI-040](../OPEN_ITEMS.md#oi-040).

<a id="err-020"></a>

## ERR-020

- Error ID: ERR-020
- Discovery time: 2026-11-27T21:13:42Z
- Related PR: GitHub PR #34, plan 027, head `e08cf7ee2dfe42303276a0426d7cec46a6f02867`
- Environment: GitHub Actions `windows-2022`, Windows Server 2022
- Reproduction method: `cargo test --locked --workspace` reached `editor_languages_debug_triggers_logs_permissions_and_empty_presets`
- Evidence: the test panicked at `plugins/strata-script/tests/workbench.rs:237` with `isolation shim is not beside the script program`. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46068679662/job/140740093587). Linux workspace checks on that head passed. The published `winstrata/merge-policy` status failed with “Plan 022 must merge before plan 027”. That order result is expected and is not this defect.
- Root cause: the test required `winstrata_shim.dll` beside the script program. `cargo test` does not build the cdylib-only shim package. The Windows job does not run the workspace build that places `libwinstrata_shim.so` beside the program on Linux. Windows containment does not load this file.
- Impact: Windows workspace checks failed before a script ran. Workspace checks then failed because Windows failed.
- Fix plan: require the shim file only on Unix, where Landlock uses it. Keep the Windows workbench assertions. Do not mark [OI-041](../OPEN_ITEMS.md#oi-041) passed.
- Retest result: not run. The next Windows workspace job on the fix commit is the retest.
- Current state: open
