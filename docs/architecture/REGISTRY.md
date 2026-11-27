# Registry plugin

[简体中文](../zh-CN/architecture/REGISTRY.md) · [Plugin host](PLUGINS.md) · [Plan 009 watch](FILE-REGISTRY.md) · [Plan 019](../roadmap/PLAN.md#pr-019)

Plan 019 is `strata.registry`. It watches current-user and local-machine registry values from the plugin host process and publishes before/after evidence on the existing event bus. It does not load `strata-reg.sys`. It does not replace the plan 009 watch, which still observes one chosen directory and one `HKEY_CURRENT_USER` subkey. Environment variables and PATH are [plan 020](ENV.md) and are not this plugin. The file plugin remains plan 021. Startup Run keys are not this plugin's purpose.

## In-process host

The native frame protocol cannot carry a before/after value and a child process cannot push an event. The watch therefore stays inside `PluginHost`. Enabling `strata.registry` does not write an installed package and does not start `winstrata-runtime`. `load`, `update`, `pin`, `grant`, `call`, and `ping` return `builtin_closed`. `uninstall` still returns `builtin_retained`, the same as the other official identities. `strata.env` is the separate plan 020 watch. `strata.trace` is the separate plan 021 watch. Official identities other than `strata.process`, `strata.registry`, `strata.env`, and `strata.trace` still report `implementation_absent`.

`host.json` stores `registry_enabled`, `registry_availability`, and `registry_scope` beside the installed entries. Missing fields read as disabled, `ready`, and an empty scope. An empty scope means the product default. An invalid stored scope makes `open` fail with `invalid_record`. Reopening an enabled plugin does not start the watch. The next `start` or `enable_batch` does, and only when the saved availability is `ready`. A saved `unsupported` result is not retried by the batch.

`enable` records the plugin as enabled and then starts the watch. On Linux the start returns `unsupported`, publishes nothing, and the report stays enabled and `unavailable`. `disable` stops the thread, clears enabled, and returns availability to `ready`. `stop` leaves the plugin enabled. A change that happens while the watch is stopped is not published, and the next start takes a new baseline. `pause` keeps the thread. A change during that pause publishes one `paused` gap for that key and that pause generation, then advances the baseline without a per-value diff. `resume` continues.

The report kind is `builtin`, the version is 0.1.0, the process id is empty, the grant is `event.publish`, and batch start is allowed. The phase is `disabled`, `stopped`, `running`, `paused`, or `unavailable`.

## Scope

The watch holds at most four keys. The set must include at least one `HKCU` key and one `HKLM` key. Hive text is exactly `HKCU` or `HKLM`. Other users' `HKU` hives are outside this plan. A subkey uses the plan 009 rule: 1 to 8 components, at most 200 bytes, no empty component, `.`, `..`, `/`, `:`, or control character, and no leading or trailing backslash. Duplicate keys are compared without case. A rejected path is not copied into the error. The detail is the fixed string `invalid registry scope`.

The product default is `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Shell Folders` and `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion`. A scope change is rejected with `running` while the watch is running. The next start uses the saved scope.

The existing plugins page lists this availability through the host report. This plan does not add a page, a query command, or a visual system. A GUI query remains part of E2E-02 and is not this acceptance.

## Events

The device is `strata.registry`. The provider is `windows.reg_notify`. The epoch is `reg-N` and increases on each start attempt. The sequence starts at 1 and is shared by observations and gaps. `precision_ns` is omitted. The process field is omitted. These APIs do not report the precision of the change time or the acting process. The user is the current process identity when that identity fits an identifier. Otherwise the user is omitted.

A subject is `HKCU\{subkey}\{name}` or `HKLM\{subkey}\{name}`. The evidence grammar matches plan 009: `t={type};a={hex}` for an addition, `t={type};b={hex}` for a removal, `t={type};b={before};a={after}` when the type stays the same, and `tb`, `ta`, `b`, and `a` when the type changes. Hex is lowercase. The evidence must fit an identifier. It is not truncated. When it does not fit, the evidence is the provider name and the partial reason is `value_exceeds_identifier`.

Each key is opened with `KEY_QUERY_VALUE | KEY_NOTIFY | KEY_WOW64_64KEY`. `RegNotifyChangeKeyValue` is asynchronous and not recursive. One thread waits on the stop event and the live notify events. It re-arms before the next snapshot. A snapshot is incomplete at 64 values, when a name or value needs more than 256 UTF-16 characters or 1024 bytes, when a name is not UTF-16, or when a name repeats. An incomplete snapshot publishes one `registry.key_changed` with `snapshot_incomplete` and does not invent a value diff. An empty diff publishes nothing. The baseline present at start is not published. If the notify is already signaled between that baseline and the first arm, the thread publishes one `lost` gap and takes a new snapshot before `start` returns. A hard failure publishes `source_failure` and drops that key. If every key fails, the thread exits. The query ring keeps the latest 128 events.

## Limits that remain

No signed registry callback was loaded. Secure Boot and HVCI were not exercised. Windows 10 and Windows 11 clients were not tested by this change. The Windows tests are compiled for `x86_64-pc-windows-msvc` and run in the Windows workspace job. That job is Windows Server 2022. The Linux authoring host cannot open a real registry. E2E-02 still requires the GUI query, files, other users, and a chosen disk. Plan 020 watches environment and PATH in [Environment plugin](ENV.md) and does not replace this plugin. Plan 021 is the file plugin in [Trace plugin](TRACE.md) and does not replace this plugin.
