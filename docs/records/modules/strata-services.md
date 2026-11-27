# Services plugin error register

[简体中文](../../zh-CN/records/modules/strata-services.md) · [Global register](../ERRORS.md)

Authoritative error records for `plugins/strata-services`.

<a id="err-020"></a>

## ERR-020

- Error ID: ERR-020
- Discovery time: 2026-11-27T20:37:52Z
- Related PR: GitHub PR #30, plan 022, head `20b7bb1502fafde469dbec45d02294ecd1d21d0a`
- Environment: GitHub Actions `windows-2022`, Windows Server 2022
- Reproduction method: `cargo test --locked --workspace`, test `service_control_manager_observes_a_driver_and_denies_a_restricted_token`
- Evidence: the restricted token called `capture`. It returned `Os { code: 5, kind: PermissionDenied, message: "Access is denied." }` instead of a snapshot with `permission_denied`. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46064710156/job/140727118812). The same job passed `missing_image_probe_records_creation_start_type_and_deletion`. Linux workspace checks passed. Workspace checks failed because Windows failed.
- Root cause: `capture` reads `MachineGuid` before it opens the Service Control Manager. The restricted token is denied that registry read, so the manager is never opened and the permission gap is not recorded.
- Impact: Windows workspace checks failed. An access-denied enumeration was not observed as `service.permission`.
- Fix plan: an access-denied identity read still opens the manager. Manager access denied stays `permission_denied` with an empty inventory. If the manager opens, the service and driver rows are kept and the snapshot also records `identity:5`. That gap does not replace an epoch already known to the tracker.
- Retest result: On head `1987f4826f92aaf5794d4678d3d4b43872932a97`, Windows Server 2022 passed `service_control_manager_observes_a_driver_and_denies_a_restricted_token`. The restricted capture set `permission_denied` and returned an empty inventory. A later unrestricted capture still saw a driver. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46068007014/job/140737899687). The same job failed `missing_image_probe_records_creation_start_type_and_deletion`. That failure is [ERR-021](#err-021) and does not reopen this defect.
- Current state: resolved

<a id="err-021"></a>

## ERR-021

- Error ID: ERR-021
- Discovery time: 2026-11-27T21:07:32Z
- Related PR: GitHub PR #30, plan 022, head `1987f4826f92aaf5794d4678d3d4b43872932a97`
- Environment: GitHub Actions `windows-2022`, Windows Server 2022
- Reproduction method: `cargo test --locked --workspace`, test `missing_image_probe_records_creation_start_type_and_deletion`
- Evidence: the assertion at `plugins/strata-services/tests/scm.rs:161` expected one `service.start-type` event for `WinStrata022Probe` and counted 0. `ChangeServiceConfigW` had returned success. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46068007014/job/140737899687). The same binary passed `service_control_manager_observes_a_driver_and_denies_a_restricted_token`. Linux workspace checks passed. Workspace checks failed because Windows failed. [Workspace job](https://github.com/zemeng5208/winstrata/actions/runs/46068007014/job/140738578743). The published `winstrata/merge-policy` status succeeded. [Policy status](https://github.com/zemeng5208/winstrata/actions/runs/46068030319).
- Root cause: `OpenServiceW` was given the service name from `wide`, and `wide` did not write a terminating NUL. The service name itself has no NUL. When the bytes after that buffer were not already zero, the open failed, `start_type` stayed `Unknown`, and the next sample compared equal, so the tracker emitted no `service.start-type`. The same missing terminator explains the earlier pass on `20b7bb1502fafde469dbec45d02294ecd1d21d0a`: a zero heap byte let the open succeed. The config buffer was also a `Vec<u8>` for a struct that contains pointers. That read was undefined. The failing job did not record `config_error`; the missing NUL is the defect that produces this count.
- Impact: Windows workspace checks and Workspace checks failed on that head. The merge stays blocked until this Windows probe passes again. [OI-043](../OPEN_ITEMS.md#oi-043) stays open. E2E-03 and E2E-14 stay open. This is not [ERR-019](../ERRORS.md#err-019) and it does not reopen [ERR-020](#err-020). Plan 025 held OI-040, OI-041, and OI-042 when OI-043 was numbered. The open gaps are now OI-044 and OI-045. OI-042 stays closed for the Server 2022 health job.
- Fix plan: `wide` appends a NUL before `OpenServiceW`. The config buffer is allocated as `u64`. The probe test still captures once after `ChangeServiceConfigW` and still requires exactly one `service.start-type` for `WinStrata022Probe`. The assertion is not removed.
- Retest result: not run. This Linux authoring host cannot open the Service Control Manager.
- Current state: open
