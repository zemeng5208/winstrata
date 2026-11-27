# Services plugin error register

[简体中文](../../zh-CN/records/modules/strata-services.md) · [Global register](../ERRORS.md)

Authoritative error records for `plugins/strata-services`.

<a id="err-019"></a>

## ERR-019

- Error ID: ERR-019
- Discovery time: 2026-11-27T20:37:52Z
- Related PR: GitHub PR #30, plan 022, head `20b7bb1502fafde469dbec45d02294ecd1d21d0a`
- Environment: GitHub Actions `windows-2022`, Windows Server 2022
- Reproduction method: `cargo test --locked --workspace`, test `service_control_manager_observes_a_driver_and_denies_a_restricted_token`
- Evidence: the restricted token called `capture`. It returned `Os { code: 5, kind: PermissionDenied, message: "Access is denied." }` instead of a snapshot with `permission_denied`. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/46064710156/job/140727118812). The same job passed `missing_image_probe_records_creation_start_type_and_deletion`. Linux workspace checks passed. Workspace checks failed because Windows failed.
- Root cause: `capture` reads `MachineGuid` before it opens the Service Control Manager. The restricted token is denied that registry read, so the manager is never opened and the permission gap is not recorded.
- Impact: Windows workspace checks failed. An access-denied enumeration was not observed as `service.permission`.
- Fix plan: an access-denied identity read still opens the manager. Manager access denied stays `permission_denied` with an empty inventory. If the manager opens while the identity read was denied, the snapshot is `identity:5` and does not publish rows under an unknown boot id.
- Retest result: not run. The Windows target type-check on this Linux host succeeded. The Windows test has not executed again.
- Current state: open
