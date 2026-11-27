# 服务插件

[English](../../architecture/SERVICES.md) · [插件宿主](PLUGINS.md) · [进程来源](PROCESS.md) · [服务控制面](SERVICE-KERNEL.md) · [计划 022](../roadmap/PLAN.md#pr-022)

计划 022 是 `plugins/strata-services`。程序名是 `winstrata-strata-services`。插件 id 是 `strata.services`。它复用原生帧协议。它不新增帧操作，也不新增外部 crate。`windows-sys` 0.61.2 已经批准，并且只在 Windows 上使用。

## 登记

只有当 `winstrata-strata-services` 是运行时旁边的普通文件时，`PluginHost` 才登记 `strata.services`。这条记录是内置插件，版本 1.0.0，在启用前保持禁用。授予的权限是 `event.publish` 和 `storage.private`。`uninstall` 保持 `builtin_retained`。`strata.process` 和 `strata.registry` 的采集不变。

采集器是只读的。它不创建、不启动、不停止、不删除服务，也不加载驱动。`CatalogHost` 仍返回 `image_absent`。[OI-014](../records/OPEN_ITEMS.md#oi-014) 保持开放。

官方外壳已经列出这个插件。本计划不增加页面、主题或视觉体系。[GUI_SOURCES.md](../records/GUI_SOURCES.md) 没有变化。

## 采集

事件写入私有的 `events.jsonl`。它们不通过 `emit` 发布。对 `services.observe` 的 `invoke` 返回采样次数和事件条数。暂停停止采样。禁用和停止会等待采样线程结束，把字节 `joined` 写入 `released`，然后以 0 退出。

## 来源

Windows 上的提供者是 `windows.scm`，Linux 上的提供者是 `procfs`。设备是 `strata.services`。省略 `precision_ns`、用户和进程。pid 留在证据里，不是 `ProcessRef`。

Windows 使用 `EnumServicesStatusExW` 读取 `SERVICE_WIN32` 和 `SERVICE_DRIVER`，并使用 `QueryServiceConfigW`。管理器拒绝访问时，记一条 `SourceFailure` 缺口，再加一条 `service.permission` 事件，并且不删除先前的行。单个服务的配置失败时保留状态，并把该事件标成部分，原因是 `config-unavailable`。

Linux 读取正在运行且以 `.service` 结尾的 cgroup 单元，跳过 `user@*.service`，并读取 `/proc/modules`。它不调用 `systemctl`，也不把单元文件当作正在运行。空列表是成功的空观察。cgroup 不可读时跳过服务差异。`/proc/modules` 返回 `EACCES` 时不发出 `driver.unloaded`。这台编写主机的 pid 1 是 `tini`。这不是权限拒绝。

## 事件

第一次采样对已经存在的行发出 `service.state` 或 `driver.state`。已经停止的服务不记成停止。已经安装的服务不记成创建。之后的变化使用 `service.created`、`service.deleted`、`service.started`、`service.stopped`、`service.start-type`、`service.config`、`service.failure`、`driver.loaded`、`driver.unloaded` 和 `driver.removed`。

`service.stopped` 是计划 010 和计划 018 延后的停止记录。它只发出一次。被截断的采样不编造删除。

Linux 只覆盖当前活动集合。消失的服务是 `service.stopped`，不是 `service.deleted`。消失的模块是 `driver.unloaded`。Windows 覆盖完整清单。缺失的服务是 `service.deleted`；如果它原先不是停止状态，同一次缺失还发出 `service.stopped`。缺失的驱动是 `driver.removed`；如果它原先正在运行，同一次缺失还发出 `driver.unloaded`。

## 仍保留的限制

启动项和计划任务仍属于计划 023。E2E-03 和 E2E-14 未完成。Windows 10 和 Windows 11、已签名驱动加载、Secure Boot 和 HVCI 均未运行。
