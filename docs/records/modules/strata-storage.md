# StrataStorage error register

[简体中文](../../zh-CN/records/modules/strata-storage.md) · [Global register](../ERRORS.md) · [Storage plugin](../../architecture/STORAGE.md)

Authoritative error records for `plugins/strata-storage`.

[ERR-025](#err-025) is resolved only for the Windows retest on head `2b4a20271a626607cdff60f1b97db62de930cda7`, job 140822369803. [OI-058](../OPEN_ITEMS.md#oi-058) is closed only for that same head and job. Job 140803723177 does not close it. A later head does not inherit either close. The Windows storage page is [OI-059](../OPEN_ITEMS.md#oi-059). The tray warning is [OI-057](../OPEN_ITEMS.md#oi-057). MCP configuration is [OI-060](../OPEN_ITEMS.md#oi-060). [OI-024](../OPEN_ITEMS.md#oi-024) stays a pending owner decision. [OI-051](../OPEN_ITEMS.md#oi-051) stays plan 026. This file does not rewrite them. [ERR-019](../ERRORS.md#err-019) through [ERR-024](strata-script.md#err-024) stay as already recorded.

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
- Retest result: resolved only for the Windows retest on head `2b4a20271a626607cdff60f1b97db62de930cda7`. Windows Server 2022 job 140822369803 in run 46094411828 completed success at 2026-11-28T01:44:07Z. The log shows `thresholds_survive_restart_and_confirmed_deletion_updates_indexes ... ok`. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46094411828/job/140822369803). The Linux pass and the Windows type-check do not close this record. This retest does not close [OI-057](../OPEN_ITEMS.md#oi-057), [OI-059](../OPEN_ITEMS.md#oi-059), [OI-060](../OPEN_ITEMS.md#oi-060), or [OI-024](../OPEN_ITEMS.md#oi-024). A later head does not inherit this job.
- Current state: resolved
