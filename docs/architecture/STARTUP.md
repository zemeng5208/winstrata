# Startup plugin

[简体中文](../zh-CN/architecture/STARTUP.md) · [Plugin host](PLUGINS.md) · [Services plugin](SERVICES.md) · [Registry plugin](REGISTRY.md) · [Plan 023](../roadmap/PLAN.md#pr-023)

Plan 023 is `plugins/strata-startup`. The binary is `winstrata-strata-startup`. The plugin id is `strata.startup`. It reuses the native frame protocol. It adds no frame operation and no new external crate. `windows-sys` 0.61.2 is already approved and is used only on Windows. `libc` is the existing Unix dependency and is used only to read the passwd home.

## Registration

`PluginHost` registers `strata.startup` only when `winstrata-strata-startup` is a regular file beside the runtime. The entry is builtin, version 1.0.0, and disabled until enabled. Its grants are `event.publish` and `storage.private`. `uninstall` stays `builtin_retained`. The manifest dependencies are empty. Plans 019 and 022 are merge-order prerequisites, not runtime dependencies. Collection by `strata.services`, `strata.process`, and `strata.registry` is unchanged. Run keys are read here. Plan 019 does not watch them.

The collector is read-only. It does not create, change, or delete a Run value, a startup shortcut, a cron file, a timer, or a scheduled task. The host job allows one process, so the collector does not spawn `schtasks` or `systemctl`.

The official shell already lists the plugin. This plan adds no page, theme, or visual system. [GUI_SOURCES.md](../records/GUI_SOURCES.md) is unchanged.

## Collector

Events go to the private `events.jsonl`. They are not published with `emit`. The file rotates at 16 MiB to `events.prev.jsonl`. `invoke` of `startup.observe` returns the sample count and the event count. The sampler waits 200 ms between samples. Pause stops samples. Disable and stop join the thread, write the bytes `joined` to `released`, and exit 0. A change that does not survive until the next sample is not invented.

After each successful sample the collector writes `inventory.json`. The file is version 1 JSON: the device id and the facts. The next process resumes that file, so a change made while the collector was stopped emits `startup.changed` or `task.changed` with `before=`, and it does not emit a second baseline. A missing file starts a baseline. An empty or unreadable file emits one `SourceFailure` gap, then a fresh baseline, and does not invent removals. A failed export does not delete the previous inventory. `state.json` records the startup count and the task count. `read_state` still rejects a file larger than 1024 bytes or a file that contains a control character, so the history file is `inventory.json`, not a new frame operation.

## Sources

The device is `strata.startup`. The provider is `windows.startup` when the snapshot evidence is `windows`, and `posix.startup` otherwise. Coverage is `Windows` or `HostFiles`. `precision_ns`, user, and process are omitted. The subject kind is `startup` or `task`. Evidence is an `Id`: 1 to 256 bytes, with no surrounding whitespace and no control characters. A field that does not fit is dropped and the event is partial. Partial reasons are `principal-exceeds-identifier`, `location-exceeds-identifier`, `command-exceeds-identifier`, `schedule-exceeds-identifier`, and `before-exceeds-identifier`, plus the fact reasons `command-absent`, `link-unresolved`, `approved-unreadable`, `extra-actions`, and `task-unreadable`.

Windows reads, and does not write:

- `HKCU` and `HKLM` `Software\Microsoft\Windows\CurrentVersion\Run` and `RunOnce`
- `HKLM` `Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run` and `RunOnce`
- `HKCU` and `HKLM` `Software\Microsoft\Windows\CurrentVersion\Policies\Explorer\Run`
- `StartupApproved` for Run, Run32, and StartupFolder. A missing, empty, or low-bit-clear value is enabled. The first byte with the low bit clear is enabled (`0x02` and `0x06`). The low bit set is disabled (`0x03` and `0x01`). This is the observed Task Manager transition, not a Microsoft specification. An unreadable `StartupApproved` value keeps the item enabled and marks `approved-unreadable`.
- The user Startup folder `{B97D20BB-F46A-4C97-BA10-5E3608430854}` and the common Startup folder `{82A5EA35-D9CD-47C5-9629-E15D2F714E6E}`, through `SHGetKnownFolderPath` with `KF_FLAG_DONT_VERIFY` (`0x4000`)
- Task Scheduler XML under `%SystemRoot%\System32\Tasks`, UTF-16 LE with a BOM or UTF-16 that starts with `<`. `Settings.Enabled` is the task flag. The first trigger `Enabled` is not that flag. The command is the action `Command` plus `Arguments`.

A missing registry key is an empty readable scope. Access denied makes that scope unreadable. `REG_EXPAND_SZ` is compared as the raw registry text. Environment expansion belongs to plan 020. A `.lnk` contributes the MS-SHLLINK LinkInfo local path. A parse failure keeps the shortcut path and marks `link-unresolved`. A RunOnce value that disappears is `startup.removed`. The collector does not claim that the command ran. A change to a script body is not a startup-registration change. That observation belongs to plan 021.

Linux coverage is host files. It is not Windows startup. The collector environment is cleared, so `XDG_CONFIG_DIRS` is unset and system autostart is `/etc/xdg/autostart`. The user directory is the passwd home from `getpwuid_r` plus `/.config/autostart`. `HOME` and `XDG_CONFIG_HOME` are not used. A user desktop file replaces a system file with the same basename. `Hidden=true` or `X-GNOME-Autostart-enabled=false` means disabled. `OnlyShowIn` is not evaluated. Cron reads `/etc/crontab`, `/etc/cron.d`, `/etc/cron.hourly`, `/etc/cron.daily`, `/etc/cron.weekly`, `/etc/cron.monthly`, and user spools `/var/spool/cron/crontabs` and `/var/spool/cron` when those directories exist. A spool directory that exists and cannot be read marks the cron scope unreadable. Systemd timers are read from `/usr/lib/systemd/system`, `/lib/systemd/system`, `/run/systemd/system`, `/etc/systemd/system`, `/etc/systemd/user`, and `$config/systemd/user`. A timer is enabled only when a same-named marker exists in a `timers.target.wants` directory. `ExecStart` is read from the paired `.service`. The collector does not call `systemctl` or `crontab`.

Identity is the Windows `MachineGuid` plus `BootId`, or Linux `/etc/machine-id` plus `/proc/sys/kernel/random/boot_id`. An empty device is not exported. `identity:5` keeps an epoch that is already known. A boot id change does not clear remembered startup facts. A different machine id starts a new baseline and does not emit removals for the previous machine. Directory symlinks are not followed. A file symlink is read. File reads stop at 1 MiB. Text that is not UTF-8 is lossy, so that file is not treated as removed. Each kind keeps at most 1024 facts, sorted and deduplicated by id. A truncated sample does not invent removals. An unreadable startup scope emits one gap and `startup.permission`. An unreadable task scope emits `task.permission`.

## Events

The first complete sample emits `startup.state` or `task.state` for facts already present. Later samples emit `startup.added`, `startup.removed`, `startup.changed`, `startup.enabled`, `startup.disabled`, `task.added`, `task.removed`, `task.changed`, `task.enabled`, and `task.disabled`. A change event includes `before=` when the previous command fits the evidence identifier. Attribution is `Observed`.

## Limits that remain

Windows Run keys, startup folders, `StartupApproved`, shortcut local paths, and Task Scheduler were not observed on this Linux authoring host. That gap is [OI-044](../records/OPEN_ITEMS.md#oi-044). Passing the Linux autostart and timer test does not close it. Windows Server 2022 is not Windows 10 or Windows 11 ([OI-001](../records/OPEN_ITEMS.md#oi-001)). E2E-03 and E2E-14 are not complete. Service Control Manager observation stays [OI-043](../records/OPEN_ITEMS.md#oi-043). [ERR-021](../records/modules/strata-services.md#err-021) stays in the services register. This plan does not retest it and does not close it.
