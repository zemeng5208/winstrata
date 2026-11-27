# Health diagnostics

[简体中文](../zh-CN/architecture/HEALTH.md) · [Index](../INDEX.md) · [Plugin host](../../core/src/plugins/mod.rs) · [Plan 025](../roadmap/PLAN.md#pr-025)

Plan 025 reads this product's own faults, queues, latency and loss. The program is `winstrata-strata-health`. It uses the existing native frame protocol: absolute `--manifest` and `--state` paths, and one JSON frame at a time. It does not add a frame operation, a dependency, or a crate from outside the workspace.

## Registration

`PluginHost::open` registers `strata.health` only when that program is a regular file in the same directory as the runtime given to the host. The entry is builtin, version 1.0.0, and disabled. Its only grant is `storage.private`. It declares `health.observe` and no events. Third-party `install` still returns `builtin_closed`. `uninstall` still returns `builtin_retained`. Without the program, `enable` still returns `implementation_absent`.

Enable starts the diagnostic process. The host does not grant `event.subscribe` or `event.publish`, so enabling it does not add a bus subscriber and does not publish observations. Pause and resume keep the process. Pause stops new lines in the private report and still returns the current reading. Disable and stop reply, write the bytes `joined` to `released`, and exit 0. There is no sampler thread to join.

## Reading

`call` of `health.observe` ignores the caller payload and sends a host snapshot. The snapshot is taken when the call is made. It contains:

| Field | Source |
|---|---|
| `plugin_faults` | Installed entries whose availability is `faulted` or `unavailable` |
| `bus_queued`, `bus_pending`, `bus_global`, `lost`, `filtered`, `queue_rejects` | `BusStats` of the bus passed to `attach_bus`, or `bus=absent` |
| `sched_queued`, `sched_inflight`, `sched_rejected`, `sched_timed_out`, `sched_panicked` | `SchedulerStats` of the handle passed to `attach_scheduler`, or `sched=absent` |
| `sent_ns` | Host clock when the snapshot is built |

`Scheduler::handle` clones the counter handle only. Dropping the handle does not shut the scheduler down and does not cancel tasks. Reading stats does not change queue admission, loss, or worker behavior.

The process rejects a snapshot that is missing a required field, repeats a field, pads a number, or pairs `absent` with that source's counters. It does not replace an absent bus or scheduler with zero. Reported `faults` are plugin faults plus scheduler timeouts and panics when the scheduler is present. Reported `queues` add the present bus and scheduler queue depths. Reported `loss` adds bus loss, filtering, queue rejects, and scheduler rejection when those sources are present. `latency_ns` is the process clock minus `sent_ns`, or zero when the process clock is earlier. The same line is appended to `diagnostics.jsonl` in the private state directory. At 1 MiB the log is renamed to `diagnostics.prev.jsonl` before the next append, and one previous generation is kept.

## What this plan does not do

StrataBudget still owns dynamic limits, pause and resume under pressure, and loss marking for that regulation. StrataPulse still owns CPU, GPU, memory and hardware counters. Service and driver snapshots, hybrid capacity, drive health, and a GUI page for these readings are not in this plan. File and registry collectors keep the separate buses created by plan 009. This plan does not attach those buses and does not change what those collectors publish. The official shell already lists this plugin. This plan does not add a page, a theme, or a visual system. First-launch enablement remains plan 017. E2E-08 and E2E-15 are not complete.

## Verification

The evidence is in the [engineering record](../records/STATUS.md). Gaps that this plan does not close are [OI-040](../records/OPEN_ITEMS.md#oi-040), [OI-041](../records/OPEN_ITEMS.md#oi-041), and [OI-042](../records/OPEN_ITEMS.md#oi-042).
