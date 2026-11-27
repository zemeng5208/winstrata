# Services plugin

[简体中文](../zh-CN/architecture/SERVICES.md) · [Plugin host](PLUGINS.md) · [Process sources](PROCESS.md) · [Service control plane](SERVICE-KERNEL.md) · [Plan 022](../roadmap/PLAN.md#pr-022)

Plan 022 is `plugins/strata-services`. The binary is `winstrata-strata-services`. The plugin id is `strata.services`. It reuses the native frame protocol. It adds no frame operation and no new external crate. `windows-sys` 0.61.2 is already approved and is used only on Windows.

## Registration

`PluginHost` registers `strata.services` only when `winstrata-strata-services` is a regular file beside the runtime. The entry is builtin, version 1.0.0, and disabled until enabled. Its grants are `event.publish` and `storage.private`. `uninstall` stays `builtin_retained`. Collection by `strata.process` and `strata.registry` is unchanged.

The collector is read-only. It does not create, start, stop, or delete a service, and it does not load a driver. `CatalogHost` still returns `image_absent`. [OI-014](../records/OPEN_ITEMS.md#oi-014) stays open.

The official shell already lists the plugin. This plan adds no page, theme, or visual system. [GUI_SOURCES.md](../records/GUI_SOURCES.md) is unchanged.

## Collector

Events go to the private `events.jsonl`. They are not published with `emit`. `invoke` of `services.observe` returns the sample count and the event count. Pause stops samples. Disable and stop join the thread, write the bytes `joined` to `released`, and exit 0.

## Sources

The provider is `windows.scm` on Windows and `procfs` on Linux. The device is `strata.services`. `precision_ns`, user, and process are omitted. A pid stays inside evidence, not a `ProcessRef`.

Windows uses `EnumServicesStatusExW` for `SERVICE_WIN32` and `SERVICE_DRIVER`, and `QueryServiceConfigW`. Access denied on the manager is a `SourceFailure` gap plus one `service.permission` event, and that failure does not delete prior rows. A per-service config failure keeps the status and marks the event partial with `config-unavailable`.

Linux reads running cgroup units that end in `.service`, skipping `user@*.service`, and reads `/proc/modules`. It does not call `systemctl`, and it does not treat unit files as running. Empty lists are successful empty observations. Unreadable cgroups skip the service diff. `EACCES` on `/proc/modules` does not emit `driver.unloaded`. This authoring host's pid 1 is `tini`. That is not a permission denial.

## Events

The first sample emits `service.state` or `driver.state` for rows already present. An already stopped service is not recorded as a stop. An already installed service is not recorded as a creation. Later changes use `service.created`, `service.deleted`, `service.started`, `service.stopped`, `service.start-type`, `service.config`, `service.failure`, `driver.loaded`, `driver.unloaded`, and `driver.removed`.

`service.stopped` is the stop record deferred by plans 010 and 018. It is emitted once. A truncated sample does not invent deletions.

Linux is active-only. A disappeared service is `service.stopped`, not `service.deleted`. A disappeared module is `driver.unloaded`. Windows is a complete inventory. A missing service is `service.deleted`, plus `service.stopped` when it was not already stopped. A missing driver is `driver.removed`, plus `driver.unloaded` when it was running.

## Limits that remain

Startup items and scheduled tasks remain plan 023. E2E-03 and E2E-14 are not complete. Windows 10 and Windows 11, signed driver load, Secure Boot, and HVCI were not run.
