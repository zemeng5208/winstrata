# Resource budgets

[简体中文](../zh-CN/architecture/BUDGET.md) · [Index](../INDEX.md) · [Plugin host](../../core/src/plugins/mod.rs) · [Plan 026](../roadmap/PLAN.md#pr-026)

Plan 026 adjusts CPU, memory, and queue limits under measured pressure, pauses and resumes `emit`, and marks loss. The program is `winstrata-strata-budget`. It uses the existing native frame protocol: absolute `--manifest` and `--state` paths, and one JSON frame at a time. It does not add a frame operation, a permission, or a crate from outside the workspace. The Windows reading uses the pinned `windows-sys` 0.61.2 feature `Win32_System_ProcessStatus`, the same crate version StrataPulse already links.

## Registration

`PluginHost::open` registers `strata.budget` only when that program is a regular file in the same directory as the runtime given to the host. The entry is builtin, version 1.0.0, and disabled. Its only grant is `storage.private`. It declares `budget.regulate` and no events. Third-party `install` still returns `builtin_closed`. `uninstall` still returns `builtin_retained`. Without the program, `enable` still returns `implementation_absent`.

Enable starts the regulator process. The host does not grant `event.subscribe` or `event.publish`. Pause and resume of the plugin frame keep the process. A frame pause returns `action=keep` and does not append a regulation line or apply a new limit. Disable and stop reply, write the bytes `joined` to `released`, and exit 0. A new host starts unpaused. The pause flag is not stored in `host.json`.

## Limits

Plan 007 ceilings stay in place. This plan does not raise workers, queues, subscriptions, admission, or batch size. The adjustable budgets cannot exceed 1,000,000 CPU ppm, 4,194,304 KiB of memory, or 65,536 queued events. Those three defaults equal the ceilings, so a normal process holds. Zero is a valid tight limit.

A caller of `budget.regulate` may send `cpu_ppm`, `mem_kib`, `queue`, `plugin`, and `plugin_queue`. An empty payload keeps the current limits. `plugin=none` clears the single plugin target. `plugin=<id>` with `plugin_queue=<n>` stores one plugin queue budget. `keep` and `none` are sentinels, not plugin ids. A measurement key from the caller is `invalid_input`. A value above a ceiling is `invalid_limit`. A padded number is rejected.

## Pressure

The host replaces the caller payload with one snapshot of at most 1,024 bytes. It reads this process, not a number supplied by the caller. On Linux the resident set is `VmRSS` from `/proc/self/status`, and CPU time is `utime` plus `stime` from `/proc/self/stat` after the last `)`, divided by `sysconf(_SC_CLK_TCK)`. On Windows the resident set is `GetProcessMemoryInfo` `WorkingSetSize`, and CPU time is `GetProcessTimes` in 100 ns units. Any other operating system reports those readings absent. An absent reading is not replaced with zero.

The first CPU sample is a baseline and does not pause. A later sample is parts per million of elapsed wall time. The measured value is not capped at 1,000,000, because more than one thread can run at once. The limit ceiling stays 1,000,000. Queue pressure is the sum of the attached bus queue and the attached scheduler queue when that source is present. A missing bus or scheduler stays absent and is left out of the sum. One plugin budget is that plugin's subscription backlog, and only while that plugin is running.

The regulator compares the snapshot with the stored limits. Queue pressure wins, then memory, then CPU, then the one plugin backlog. A present reading above its limit pauses. Entering that pause sets `loss=1`. The same pause with an unchanged bus loss count sets `loss=0`. If bus loss increases while the regulator is still paused, `loss` is 1 again. When every present reading is within its limit, a pause that is in force resumes, and otherwise the action is hold. The same line is appended to `regulation.jsonl` in the private state directory. At 1 MiB the log is renamed to `regulation.prev.jsonl` before the next append, and one previous generation is kept.

The host publishes the loss marker. The plugin does not. The marker is a protocol `Record::Gap` with `GapReason::Lost` and no range, on device `local-plugin-host`, provider `strata.budget`, the host epoch, and a sequence starting at 1. A repeated pause does not invent another hole. An earlier overflow gap on another source stays `Lost`. The host sets `budget_paused` on pause and clears it on hold or resume. While that flag is set, `emit` returns `budget_paused` before the grant check. `EventBus::publish` is not stopped. After resume, `emit` from this plugin still returns `permission_denied`, because the manifest does not declare `event.publish`.

## What this plan does not do

Disk I/O, network, storage, and GUI refresh budgets are not implemented and are not reported as zero. That gap is [OI-051](../records/OPEN_ITEMS.md#oi-051). This plan does not read another process's CPU or resident set. Service and driver snapshots, drive health, and a GUI page stay outside this plan, in [OI-047](../records/OPEN_ITEMS.md#oi-047). File and registry collectors keep the separate buses from plan 009. This plan does not attach those buses. [OI-048](../records/OPEN_ITEMS.md#oi-048) stays open. Sustained pressure remains plan 048. The official shell already lists this plugin. This plan does not add a page, a theme, or a visual system. [GUI_SOURCES.md](../records/GUI_SOURCES.md) is unchanged. First-launch enablement remains plan 017. E2E-08 and E2E-15 are not complete.

## Verification

The evidence is in the [engineering record](../records/STATUS.md). The Windows tests were not run on the Linux authoring host. That gap is [OI-050](../records/OPEN_ITEMS.md#oi-050). It does not close [OI-001](../records/OPEN_ITEMS.md#oi-001).
