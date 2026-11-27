# Pulse counters

[简体中文](../zh-CN/architecture/PULSE.md) · [Collector](../../plugins/strata-pulse/src/main.rs) · [Host](../../core/src/plugins/mod.rs) · [Plan 024](../roadmap/PLAN.md#pr-024)

Plan 024 runs StrataPulse in `plugins/strata-pulse`. The program is `winstrata-strata-pulse`. It uses the existing native frame protocol: absolute `--manifest` and `--state` paths, and one JSON frame at a time. It does not add a frame operation, a protocol permission, or a dependency.

Direct prerequisites 010, 012, 014, and 016 are already on main. This plan does not implement plans 020, 021, 022, 023, 025, 026, 027, or 032.

## Registration

<a id="plugin"></a>

`PluginHost::open` registers `strata.pulse` only when `winstrata-strata-pulse` is a regular file in the same directory as the runtime given to the host. The entry is builtin, version 1.0.0, and disabled. Its grants are `event.publish` and `storage.private`. Third-party `install` still returns `builtin_closed`. `uninstall` still returns `builtin_retained`. Without that adjacent file, `enable` still returns `implementation_absent`.

`strata.process` keeps its own adjacent-file check. `strata.registry` stays in process. Enabling pulse does not start those collectors, and their sample paths are unchanged.

## Sample

The collector samples about every 200 ms on a `ws-pulse` thread. A reading is one of `measured`, `baseline`, `unsupported`, or `failed`.

A measured reading has an integer value and a source. A baseline is a counter that has been seen once and does not yet have a delta. Unsupported means this host has no such source, or this collector has no reader for that API. Failed means the source was present and the read or the arithmetic did not produce a value. Baseline, unsupported, and failed readings have no value. A missing counter is not stored as 0.

CPU usage is the delta of busy ticks over total ticks, in parts per million. Busy excludes idle and iowait. The first sample is baseline. Memory total, available, and used come from `MemAvailable` on Linux and `GlobalMemoryStatusEx` on Windows. Used is total minus available only when both values were read and available does not exceed total. Swap total is recorded from `SwapTotal` when that line exists, including a real 0. Load is the one-minute load average in milli-units when `/proc/loadavg` exists.

Frequency, CPU temperature, hwmon, battery capacity, battery power, and RAPL energy are read from sysfs when the files exist. A 0 frequency file is not treated as a measurement. GPU busy percent and VRAM come from DRM sysfs when those files exist. An idle GPU that reports 0 percent is measured. A missing DRM counter is unsupported with reason `no_counter`.

Hardware performance counters use `perf_event_open` for cycles and instructions. The collector tries each online CPU, then one counter for this process. Denial is `perf_denied`. Any other open failure is `perf_unavailable`. The first successful read is baseline. Later reads store the delta. A partial CPU set uses source `perf.cpu.partial` and records how many CPUs opened.

Process count, the eight highest RSS values, and the eight highest CPU-tick deltas are kept in the sample document. Per-process rows are not metric series. The first process-CPU sample is baseline. A process whose RSS cannot be read is omitted from the RSS list and the sample is marked limited. This collector does not emit `process.started` or any other process-lifecycle event.

On Windows the same document is filled from `GetSystemTimes`, `GlobalMemoryStatusEx`, `CallNtPowerInformation`, `GetSystemPowerStatus`, and Toolhelp plus `GetProcessTimes` and `GetProcessMemoryInfo`. Load, swap, GPU, VRAM, temperature, RAPL, and perf stay unsupported with reason `reader_absent` or `no_loadavg` or `no_swap`. `reader_absent` means this collector does not call that API. It does not mean the device reported the counter missing. Battery percent 255 and the no-battery flag stay unsupported and have no value. [OI-040](../records/OPEN_ITEMS.md#oi-040) tracks the Windows GPU, VRAM, and thermal readers.

Drive SMART, logical and physical writes, and wear remain plan 032. This collector does not read them and does not invent them.

## History

Each retained sample appends one JSON object to `history.jsonl` in the private state directory and replaces `latest.json`. At 16 MiB the log is renamed to `history.prev.jsonl` before the next append, and one previous generation is kept. `peaks.json` stores the maximum measured value of each series. Unsupported and baseline readings do not move a peak. `samples` is the retained sample count. Pause stops new samples and leaves the process running. Disable and stop join the `ws-pulse` thread, write `joined` to `released`, and exit 0.

Measured values are also committed through `Core::commit_plugin_metrics` into the plan 012 metric store. On Windows the canonical `\\?\` prefix is removed before the workspace anchor is opened. The series id is `pulse.` plus the reading id, for example `pulse.memory.total.bytes` and `pulse.cpu.usage.ppm`. The state directory must be `<user>/plugins/state/strata.pulse`. If `vault.wsv` is present the commit is refused, the sample records `store` as `locked`, and the private history is still written. A sample with no measured value records `private_only` and does not open the store. These observations are not published with `emit`. `invoke` of `pulse.observe` returns the latest sample, measured, and unsupported counts.

The official shell already lists this plugin. This plan does not add a page, a chart, a theme, or a visual system. [GUI_SOURCES.md](../records/GUI_SOURCES.md) is unchanged. First-launch enablement remains plan 017. StrataHealth and StrataBudget remain plans 025 and 026.

## Verification

On this Linux x86_64 host the pulse tests read `/proc/stat`, `/proc/meminfo`, and `/proc/loadavg`. This host has no DRM GPU counter, cpufreq, thermal zone, hwmon, battery, or RAPL directory, so those readings are unsupported. The observe test required a measured memory total, available memory no greater than that total, used memory equal to the difference, a CPU delta from 0 through 1,000,000 ppm, a process count of at least one, a baseline CPU reading with no value in the first history line, peaks for memory total, a committed hybrid series `pulse.memory.total.bytes`, and no hybrid series for an unsupported GPU reading. Pause held the sample count. Disable wrote `joined`. Counts for the workspace run are in the [engineering record](../records/STATUS.md).

This host did not run the Windows tests. E2E-08 is not complete.
