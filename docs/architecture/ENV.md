# Environment plugin

[简体中文](../zh-CN/architecture/ENV.md) · [Plugin host](PLUGINS.md) · [Plan 020](../roadmap/PLAN.md#pr-020)

Plan 020 is `strata.env`. It watches the current-user environment store and the system environment store from the plugin host process and publishes before/after evidence on the existing event bus. It does not load a driver. It does not replace plan 009. It does not watch the plan 019 registry scope. Startup Run keys, files, services, and tasks are not this plugin.

## In-process host

The native frame protocol cannot carry a before/after value and a child process cannot push an event. The watch stays inside `PluginHost`. Enabling `strata.env` does not write an installed package and does not start `winstrata-runtime`. `load`, `update`, `pin`, `grant`, `call`, and `ping` return `builtin_closed`. `uninstall` returns `builtin_retained`. The registry plugin remains the plan 019 watch. The other official identities that still have no collector stay `implementation_absent`.

`host.json` stores `env_enabled`, `env_availability`, and `env_scope` beside the installed entries. Missing fields read as disabled, `ready`, and an empty scope. An empty scope means the product default. An invalid stored scope makes `open` fail with `invalid_record`. Reopening an enabled plugin does not start the watch. The next `start` or `enable_batch` does, and only when the saved availability is `ready`. A saved `unsupported` result is not retried by the batch.

`enable` records the plugin as enabled and then starts the watch. On Linux the start returns `unsupported`, publishes nothing, and the report stays enabled and `unavailable`. `disable` stops the thread, clears enabled, and returns availability to `ready`. `stop` leaves the plugin enabled. A change while the watch is stopped is not published, and the next start takes a new baseline. `pause` keeps the thread. A change during that pause publishes one `paused` gap for that store and that pause generation, then advances the baseline without a per-value diff. `resume` continues.

The report kind is `builtin`, the version is 0.1.0, the process id is empty, the grant is `event.publish`, and batch start is allowed. The phase is `disabled`, `stopped`, `running`, `paused`, or `unavailable`.

## Scope

The scope is exactly two stores. One must be `HKCU` and one `HKLM`. Hive text is exactly `HKCU` or `HKLM`. Subkeys use the plan 009 rule: 1 to 8 components, at most 200 bytes, no empty component, `.`, `..`, `/`, `:`, or control character, and no leading or trailing backslash. Duplicate keys are compared without case. A rejected path is not copied into the error. The detail is the fixed string `invalid env scope`.

The product default is `HKCU\Environment` and `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment`. A scope change is rejected with `running` while the watch is running. The next start uses the saved scope.

The existing plugins page lists this availability through the host report. This plan does not add a page, a query command, or a visual system. It does not add Fluent UI or Astryx. A GUI query remains part of E2E-02 and is not this acceptance.

## Events

The device is `strata.env`. The provider is `windows.env_notify`. The epoch is `env-N` and increases on each start attempt. The sequence starts at 1 and is shared by observations and gaps. `precision_ns` is omitted. The process field is omitted. The user is the current process identity when that identity fits an identifier. Otherwise the user is omitted.

A subject is `user\{name}` or `system\{name}`, and the object kind is `environment`. When that text does not fit an identifier, the subject is omitted and the event is partial with reason `identifier_limit`. The name is not copied into the event. Names compare without ASCII case. `REG_SZ` and `REG_EXPAND_SZ` are text, decoded as UTF-16 and stored in evidence as lowercase hex of the UTF-8 string. Tokens are `sz` and `expand`. Addition is `t={token};a={hex}`. Removal is `t={token};b={hex}`. A same-type change is `t={token};b={before};a={after}`. A type change uses `tb`, `ta`, `b`, and `a`. The evidence must fit an identifier. It is not truncated. When it does not fit, a non-PATH value uses evidence `windows.env_notify` and partial reason `value_exceeds_identifier`.

A variable whose name is `PATH`, in any ASCII case, uses `env.path_added`, `env.path_removed`, or `env.path_changed`. Other variables use `env.variable_added`, `env.variable_removed`, or `env.variable_changed`. When the full PATH evidence does not fit, the watcher does not emit a truncated full value. It splits on semicolons, drops empty segments, and emits one complete `env.path_added` or `env.path_removed` event per added or removed segment whose own evidence fits. A segment that does not fit, or a change with no segment difference such as order only, emits one partial `env.path_changed` with reason `value_exceeds_identifier`. A non-text value emits a partial event with reason `value_not_text` and does not include the bytes.

Each store is opened with `KEY_QUERY_VALUE | KEY_NOTIFY | KEY_WOW64_64KEY`. `RegNotifyChangeKeyValue` is asynchronous and not recursive. One thread waits on the stop event and the live notify events. It re-arms before the next snapshot. A snapshot is incomplete at 256 values, when a name needs more than 256 UTF-16 code units, when a value needs more than 32768 bytes, when a name is not UTF-16, or when a name repeats ignoring case. An incomplete snapshot publishes one `env.block_changed` with `snapshot_incomplete` and does not invent a value diff. An empty diff publishes nothing. The baseline present at start is not published. If the notify is already signaled between that baseline and the first arm, the thread publishes one `lost` gap and takes a new snapshot before `start` returns. A hard failure publishes `source_failure` and drops that store. If every store fails, the thread exits. The query ring keeps the latest 128 events.

## Limits that remain

No Windows 10 or Windows 11 client was tested by this change. The Windows tests are compiled for `x86_64-pc-windows-msvc` and run in the Windows workspace job. That job is Windows Server 2022. The Linux authoring host cannot open the real user or system environment stores. E2E-02 still requires the GUI query, files, and other users. Plan 021 is not started. The Windows host test writes one probe variable and appends one PATH segment on the real user and system stores, then restores the previous bytes. The plugin crate's Windows tests use volatile keys and do not write those stores.
