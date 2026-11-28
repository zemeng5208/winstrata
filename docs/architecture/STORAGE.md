# Storage plugin

[简体中文](../zh-CN/architecture/STORAGE.md) · [Index](../INDEX.md) · [Plugin host](../../core/src/plugins/mod.rs) · [Plan 030](../roadmap/PLAN.md#pr-030)

Plan 030 is the official plugin `strata.storage` in `plugins/strata-storage`. The display name is StrataStorage. The program is `winstrata-strata-storage`. It uses the existing native frame protocol: absolute `--manifest` and `--state` paths, and one JSON frame at a time. It does not add a frame operation, a permission, or a crate from outside the workspace. Deletion stays in Core.

## Registration

`PluginHost::open` registers `strata.storage` only when `winstrata-strata-storage` is a regular file beside the runtime passed to the host. The seeded entry is builtin, version 1.0.0, and stays disabled until enabled. Its capabilities are `storage.configure`, `storage.decline`, `storage.propose`, and `storage.status`. Its permissions are `storage.private` and `plugin.call`. It declares no events. Third-party `install` still returns `builtin_closed`. `uninstall` still returns `builtin_retained`. Without the program, `enable` still returns `implementation_absent`.

Enable starts the storage process. Pause and resume keep the process. Disable and stop reply, write the bytes `joined` to `released`, and exit 0. A payload with `confirm` true, or `op` `confirm`, returns `confirmation_required` and writes nothing.

## Thresholds

Core keeps the ladder in `<user>/storage-alerts.json`, beside `core.json`. The plugin process writes only its private `policy.json`, `decline.json`, and `proposal-request.json`. On Windows those files are created with the explicit owner and protected DACL `O:<user>D:P(A;;FA;;;<user>)`, the same descriptor Core gives a private file. An elevated token would otherwise record the owner as Administrators, and `open_existing` would refuse `policy.json`. That check is unchanged. `storage_notice` adopts `policy.json` when that file already exists, then consumes one `decline.json` ticket. `Core::configure_storage` writes the same ladder. If `policy.json` already exists, it overwrites that file so the two copies do not diverge. `Core::decline_storage` moves one step and does not read the ticket.

The candidate defaults are decimal bytes: persisted captures warn at 1,000,000,000 and repeat every 1,000,000,000; saved logs warn at 2,000,000,000 and repeat every 500,000,000. A configured value is an integer from 1 through 1,000,000,000,000,000. These units are not an owner decision. [OI-024](../records/OPEN_ITEMS.md#oi-024) stays open.

Saved logs are regular files under `<user>/logs` whose names match `^[a-z][a-z0-9._-]{0,47}$` and end in `.log`. Events, metrics, indexes, backups, and structured records are not counted. Captures are the regular-file bytes under `<user>/data/captures`. The warning state is the armed threshold `first + declines * repeat`. Usage at or above that threshold is pending. One decline adds one step. If usage is still at or above the new threshold, the warning stays pending. No response does not delete and does not increment declines. Cleanup sets declines back to 0 and evaluates again against the first threshold. Changing the thresholds recalculates the same way.

## Confirmation and indexes

`propose_storage` resolves a scope and stores a proposal id. It does not delete. `confirm` true inside that request returns `confirmation_required` and stores nothing. `expiration` returns `expiration_unconfirmed` and deletes nothing. `capacity` returns `selection_required` and does not choose a segment. `confirm_storage` deletes only when the trusted caller passes `confirm` true and the proposal id still matches a scope resolved again from the current index. A mismatch returns `stale` and deletes nothing.

A segment is deleted only when every record in it matches the scope and none is protected. Otherwise it is held as `protected` or `partial`. The drop is a journal commit with empty refs and waters plus a trailer of segment pairs and the object ids that lived in structured segments. A commit with no drops omits the trailer, so an older journal still decodes. Replay skips a missing payload only when a later commit in the same applied prefix drops that segment, then sets that segment's water to 0 and forgets its index entries. Relations in a surviving segment are removed when they point at a dropped object. Category `indexes` and segment 0 cannot be dropped. After the journal is synced, the host deletes the segment files. An audit line is appended to `<user>/storage-audit.jsonl`.

Cleanup does not follow a path outside the five data segment categories and the regular files under `<user>/logs`. An unavailable store returns `storage` `unavailable` and does not delete. Core still returns the ladder when the plugin process is disabled.

## Script and page

The script workbench operation `storage` does not start an interpreter. An enabled script with a per-script `plugin.call` grant returns `code` `storage_request` for `configure`, `decline`, `propose`, and `status`. The host forwards that request to the storage process. `confirm`, or `confirm` true, returns `confirmation_required` and is not forwarded. Without the grant the workbench returns `permission_denied` and writes no file. The script plugin manifest does not gain `plugin.call`.

The official page at `/storage` composes the existing shadcn-admin [Button](../../packages/ui/vendor/shadcn-admin/src/components/ui/button.tsx), [Input](../../packages/ui/vendor/shadcn-admin/src/components/ui/input.tsx), and [Badge](../../packages/ui/vendor/shadcn-admin/src/components/ui/badge.tsx). It does not add Fluent UI, Astryx, or a new visual system. It does not send `enable` or `grant`. The bridge commands `storage-status`, `storage-configure`, `storage-decline`, `storage-propose`, and `storage-confirm` call Core. They do not enable the plugin. Before a storage location is chosen, `storage-status` returns `enabled` false and `storage` `unavailable`, and it does not create the plugins directory. Deletion uses the sibling field `confirm` true on `storage-confirm`. The tray tip stays the product name. That gap is [OI-057](../records/OPEN_ITEMS.md#oi-057). The Windows tests and the Windows page session were not run on this host. Those gaps are [OI-058](../records/OPEN_ITEMS.md#oi-058) and [OI-059](../records/OPEN_ITEMS.md#oi-059). MCP configuration is plan 045 and is [OI-060](../records/OPEN_ITEMS.md#oi-060).

## What this plan does not do

This plan does not close [OI-024](../records/OPEN_ITEMS.md#oi-024), [OI-047](../records/OPEN_ITEMS.md#oi-047), or [OI-051](../records/OPEN_ITEMS.md#oi-051). It does not implement plan 031 or a later plan. [E2E-05](../roadmap/ACCEPTANCE.md#e2e-05) and [E2E-15](../roadmap/ACCEPTANCE.md#e2e-15) stay open. The disk, network, storage, and GUI refresh budgets stay with plan 026.

## Verification

The evidence is in the [engineering record](../records/STATUS.md).
