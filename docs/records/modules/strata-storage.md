# StrataStorage error register

[简体中文](../../zh-CN/records/modules/strata-storage.md) · [Global register](../ERRORS.md) · [Storage plugin](../../architecture/STORAGE.md)

Authoritative error records for `plugins/strata-storage`.

[ERR-025](#err-025) is open. The Windows test gap remains [OI-058](../OPEN_ITEMS.md#oi-058). Job 140803723177 does not close it. The Windows storage page is [OI-059](../OPEN_ITEMS.md#oi-059). The tray warning is [OI-057](../OPEN_ITEMS.md#oi-057). MCP configuration is [OI-060](../OPEN_ITEMS.md#oi-060). [OI-024](../OPEN_ITEMS.md#oi-024) stays a pending owner decision. [OI-051](../OPEN_ITEMS.md#oi-051) stays plan 026. This file does not rewrite them. [ERR-019](../ERRORS.md#err-019) through [ERR-024](strata-script.md#err-024) stay as already recorded.

<a id="err-025"></a>

## ERR-025

- Error ID: ERR-025
- Discovery time: 2026-11-28T00:35:38Z
- Related PR: GitHub PR #39, plan 030, head `f4b27ac0f128ecf5548f9b32f0aab888112bff06`
- Environment: GitHub Actions `windows-2022`, Windows Server 2022. [Windows job 140803723177](https://github.com/zemeng5208/winstrata/actions/runs/46087953366/job/140803723177), run 46087953366 attempt 2
- Reproduction method: `cargo test --locked --workspace` reached `thresholds_survive_restart_and_confirmed_deletion_updates_indexes`
- Evidence: the test panicked at `plugins/strata-storage/tests/cleanup.rs:253` on `core.configure_storage(...)` after Core was reopened. `decline_storage("capture")` had already succeeded. The error was `PermissionDenied`, `storage must be owned by the current user with its private DACL`. Linux workspace checks on that head passed. Attempt 1, [job 140801875313](https://github.com/zemeng5208/winstrata/actions/runs/46087953366/job/140801875313), timed out in `plugins/strata-process` and did not run this test. That timeout is not this defect. This plan does not change that package. Workspace checks failed because Windows failed. The published `winstrata/merge-policy` status failed with “PR is in draft”. That draft result is expected and is not this defect.
- Root cause: the plugin created `policy.json` with `OpenOptions` and no security descriptor. `Core::configure_storage` is the call that updates an existing `policy.json` through `open_existing`. That check requires the token user as owner and the private DACL. `storage_notice` and `decline_storage` do not open that file, so they succeeded after restart. On an elevated Windows token, a file created without an explicit owner is owned by Administrators, and its DACL is inherited. This host did not read the runner’s security descriptor. The denial and the creation path match that check. The check is unchanged.
- Impact: Windows workspace checks failed inside the storage integration test. Workspace checks failed with them. [OI-058](../OPEN_ITEMS.md#oi-058) stays open because this job did not pass the test.
- Fix plan: create the plugin files with the same explicit owner and protected DACL Core uses for a private file, `O:<user>D:P(A;;FA;;;<user>)`. Do not weaken `open_existing`. Do not change plan 018.
- Retest result: not run on Windows. After the fix, the Linux authoring host passed `thresholds_survive_restart_and_confirmed_deletion_updates_indexes`. That pass does not close this record. `RUSTFLAGS="-Dwarnings" cargo check --target x86_64-pc-windows-msvc --offline -p winstrata-strata-storage` compiled the plugin. It is not a Windows test.
- Current state: open
