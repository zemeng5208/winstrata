# Process, thread and module sources

[简体中文](../zh-CN/architecture/PROCESS.md) · [Sampler](../../kernel/src/system.rs) · [Tracker](../../core/src/process.rs) · [Plan 010](../roadmap/PLAN.md#pr-010)

Plan 010 adds user-mode process sampling to `winstrata-kernel` and a process tracker to `winstrata-core`. The direct prerequisite is plan 008. `CatalogHost` still returns `image_absent` for `strata-proc` and does not load a driver. The service IPC is unchanged. Core calls the sampler in-process because reading procfs or the Windows process APIs does not require a new privileged operation.

## Sample

`capture` reads one sample for the caller's token. It does not start, stop or create a service, and it does not open a kernel process callback.

On Linux the device id is `/etc/machine-id` and the boot id is `/proc/sys/kernel/random/boot_id`. Process start time comes from `/proc/stat` `btime` plus field 22 of `/proc/<pid>/stat`, converted with `sysconf(_SC_CLK_TCK)`. Threads are `/proc/<pid>/task` entries. Modules are executable file mappings in `/proc/<pid>/maps`, keyed by path. A cgroup path contributes a service row for each `.service` unit except `user@*.service`.

On Windows the device id is `MachineGuid` and the boot id is the `BootId` registry value, prefixed with `boot:`. Processes and threads come from Toolhelp. Start time comes from `GetProcessTimes` and the image path from `QueryFullProcessImageNameW`. Modules come from a per-process module snapshot. Service rows come from `EnumServicesStatusExW` for `SERVICE_WIN32`, using the reported process id. A process id of zero is not a service association.

The sample keeps at most 4,096 processes, 1,024 threads and 64 modules per process, and 1,024 service rows. A longer list sets the corresponding truncated flag. An unreadable image, start time, thread list or module list sets `limited` and leaves the missing field empty. The sampler does not invent a start time. If service enumeration fails, `services_unavailable` is set and the service list is empty.

## Tracker

`ProcessTracker::observe` assigns protocol events on provider `winstrata.process`. The source epoch is the boot id. An instance is the boot id, PID and start time. The v1 observation has no attribute map, so the image, parent and service names remain on `ProcessNode`. Command lines are not read.

Plan 017 adds `observe_scoped`. It drops a process whose image is outside the selected roots, whose name is not selected, or whose image is missing. It also drops a service row whose process was dropped. File extensions do not filter process images. With process watching off and no selected names, it returns no events and does not call `observe`. `observe` itself is unchanged.

The first sample that contains an instance emits `process.started` with observed attribution and evidence `procfs` or `toolhelp`. A later sample that still contains that instance does not emit the start again. When the instance is absent, one `process.exited` event is emitted with partial completeness `not_in_snapshot`. Another sample does not repeat the exit. The event time is the sample time, not a kernel exit timestamp.

A PID whose start time differs from the tracked instance closes the old instance and opens a new one. A sample that is truncated does not turn a missing PID into an exit and does not replace a tracked start time. Threads and modules use the same first-seen and absence rule. A truncated thread or module list does not emit removals. Process exit does not also emit thread and module exits.

A service row whose PID has a start time emits one `process.service` event with inferred attribution and evidence `service:<name>`. That event is not a second `process.started`. The same service name is not emitted again for that instance. A service PID without a start time emits one partial `process.service` event and does not create a `ProcessRef`. A PID that remains visible without a start time emits one `process.unresolved` event until it disappears. A tracked instance that is still listed without a start time does not emit `process.exited`. That exit waits for a later complete sample that omits the instance or shows a different start time.

A parent link is kept when the parent is in the same sample and its start time is not later than the child's. A cycle is removed. A changed boot or device id closes the previous source before new events use the new epoch.

## Coverage

This is a sample, not a continuous trace. A process that starts and exits between samples produces no event, and the tracker does not invent a sequence hole for it. ETW and kernel process callbacks are not implemented. Plan 022 now records service stop records as `service.stopped` in `strata.services` ([Services plugin](SERVICES.md)). Plan 010 and plan 018 still do not emit `service.stopped`. File, registry and network collection remain plans 009 and 011. The plugin that runs this sampler is [plan 018](#plugin).

## Plugin

<a id="plugin"></a>

Plan 018 runs the sampler in `plugins/strata-process`. The program is `winstrata-strata-process`. It uses the existing native frame protocol: absolute `--manifest` and `--state` paths, and one JSON frame at a time. It does not add a frame operation or a dependency.

`PluginHost::open` registers `strata.process` only when that program is a regular file in the same directory as the runtime given to the host. The entry is builtin, version 1.0.0, and disabled. Its grants are `event.publish` and `storage.private`. Third-party `install` still returns `builtin_closed`. `uninstall` still returns `builtin_retained`. `strata.registry`, `strata.env`, and `strata.trace` are in-process watches and are no longer absent. `strata.services` is registered only when `winstrata-strata-services` is a regular file beside the runtime. `strata.startup` is registered only when `winstrata-strata-startup` is a regular file beside the runtime. The other official identities stay `implementation_absent` until their own plans.

Enable starts the collector. The process samples about every 200 ms through `capture` and `ProcessTracker`. A successful sample appends that tracker's protocol events to `events.jsonl` in the private state directory and replaces `tree.json`. The tree carries parent links, images, service names, threads, and modules. Start times and module bases are decimal strings because they do not fit a JSON number exactly. At 16 MiB the log is renamed to `events.prev.jsonl` before the next append, and one previous generation is kept. Tracker state stays in the process, so rotation does not emit a second start.

`process.started`, thread events, and module events keep observed attribution. The evidence is `procfs` or `toolhelp`. `process.service` keeps inferred attribution. The provider remains `winstrata.process`. These observations are not published with `emit`. A frame text field holds at most 1,024 bytes, and `emit` would record the collector process instead of the sampled process. `invoke` of `process.observe` returns the sample count and the event count.

Pause stops new samples and leaves the process running. Resume continues the same tracker. Disable and stop ask the sampler to exit, send the existing reply frame, join the `ws-process` thread, write the bytes `joined` to `released`, and exit 0. A panicked sampler does not write that marker, and the process exits 1. The host still deletes only a language `env` directory. The event log and the tree remain.

The official shell already lists this plugin. This plan does not add a page, a theme, or a visual system. First-launch enablement remains plan 017.

Command lines, ETW, kernel process callbacks, and a process that starts and exits between samples are not implemented. Plan 022 now records service stop records as `service.stopped` in `strata.services` ([Services plugin](SERVICES.md)). Plan 010 and plan 018 still do not emit `service.stopped`. No signed `strata-proc` image is loaded. E2E-02 and E2E-03 are not complete.

## Verification

On Linux x86_64 with Rust 1.100.0, `RUSTFLAGS="-Dwarnings" cargo test --workspace` passed 32 Core tests, 9 kernel tests, 11 protocol library tests, 2 protocol integration tests and 7 service tests. The new tests read the live process table. A real child was linked to this test process, appeared once, survived a second sample without another start, and produced one exit after it was reaped. The table contained a chain of at least three processes. The same events were published on the core bus with contiguous sequences and no recorded loss. This host's sample contained 33 processes and no `.service` cgroup unit, so the service test checked that no indirect event was invented.

The reuse test closed one real start time and opened another real start time under the same PID. That combination was arranged in the tracker input. The operating system did not recycle a PID during the test. `cargo check --target x86_64-pc-windows-msvc --tests` type-checked the Toolhelp, registry and Service Control Manager code with warnings denied. Those Windows tests were not executed on that host. Later Windows Server 2022 jobs did run `real_process_tree_pid_lifecycle_and_dedup`. It passed on the plan 010 evidence and on the plan 013 review head. Main `a2471d5fc13f0a307ee39d2629c454aea4537809` failed it. See [ERR-005](../records/modules/core.md#err-005). Plan 052 merged as `004974215e239f96160b47d3eef40a84c316e802`, and Windows workspace checks on that commit passed. The root cause of the earlier failure remains unknown. No signed `strata-proc` image was loaded. E2E-02 and E2E-03 remain open. Counts and the Windows run belong in the [engineering record](../records/STATUS.md).

Plan 052 does not change that exit rule. `real_process_tree_pid_lifecycle_and_dedup` still requires one `process.exited` event after the child is reaped. The test waits for a complete sample that omits the instance or replaces its start time. A sample that still lists the PID without a positive start time is not that event, and a truncated sample is not that event either. The main failure log did not include the sample, so the record does not claim which of those two conditions occurred on that run. Plan 051 does not change this test.

Plan 018 ran that sampler from `plugins/strata-process` on this Linux x86_64 host with Rust 1.100.0. After the restack onto `88249119943516437819d93eb7f7bcd50c3b45b8`, `RUSTFLAGS="-Dwarnings" cargo test --locked --workspace` passed 66 Core tests, 23 kernel tests, 14 protocol library tests, 2 protocol integration tests, 3 language tests, 5 runtime lifecycle tests, 7 service tests, 5 desktop tests, and 1 process-plugin test. The plugin test spawned `/bin/sleep 120` and observed one start, one thread event, one module event, a parent link in `tree.json`, one exit, and `joined` in `released` after disable. This host did not run the Windows tests. E2E-02 and E2E-03 remain open.
