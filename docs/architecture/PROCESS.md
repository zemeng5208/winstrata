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

The first sample that contains an instance emits `process.started` with observed attribution and evidence `procfs` or `toolhelp`. A later sample that still contains that instance does not emit the start again. When the instance is absent, one `process.exited` event is emitted with partial completeness `not_in_snapshot`. Another sample does not repeat the exit. The event time is the sample time, not a kernel exit timestamp.

A PID whose start time differs from the tracked instance closes the old instance and opens a new one. A sample that is truncated does not turn a missing PID into an exit and does not replace a tracked start time. Threads and modules use the same first-seen and absence rule. A truncated thread or module list does not emit removals. Process exit does not also emit thread and module exits.

A service row whose PID has a start time emits one `process.service` event with inferred attribution and evidence `service:<name>`. That event is not a second `process.started`. The same service name is not emitted again for that instance. A service PID without a start time emits one partial `process.service` event and does not create a `ProcessRef`. A PID that remains visible without a start time emits one `process.unresolved` event until it disappears.

A parent link is kept when the parent is in the same sample and its start time is not later than the child's. A cycle is removed. A changed boot or device id closes the previous source before new events use the new epoch.

## Coverage

This is a sample, not a continuous trace. A process that starts and exits between samples produces no event, and the tracker does not invent a sequence hole for it. ETW and kernel process callbacks are not implemented. Service stop and restart records remain plan 022. File, registry and network collection remain plans 009 and 011. The plugin presentation remains plan 018.

## Verification

On Linux x86_64 with Rust 1.100.0, `RUSTFLAGS="-Dwarnings" cargo test --workspace` passed 32 Core tests, 9 kernel tests, 11 protocol library tests, 2 protocol integration tests and 7 service tests. The new tests read the live process table. A real child was linked to this test process, appeared once, survived a second sample without another start, and produced one exit after it was reaped. The table contained a chain of at least three processes. The same events were published on the core bus with contiguous sequences and no recorded loss. This host's sample contained 33 processes and no `.service` cgroup unit, so the service test checked that no indirect event was invented.

The reuse test closed one real start time and opened another real start time under the same PID. That combination was arranged in the tracker input. The operating system did not recycle a PID during the test. `cargo check --target x86_64-pc-windows-msvc --tests` type-checked the Toolhelp, registry and Service Control Manager code with warnings denied. Those Windows tests were not executed. No signed `strata-proc` image was loaded. E2E-02 and E2E-03 remain open. Counts and the Windows run belong in the [engineering record](../records/STATUS.md).
