# 资源限额

[English](../../architecture/BUDGET.md) · [文档索引](../INDEX.md) · [插件宿主](../../../core/src/plugins/mod.rs) · [计划 026](../roadmap/PLAN.md#pr-026)

计划 026 在实测压力下调整 CPU、内存和队列限额，暂停并恢复 `emit`，并标记丢失。程序是 `winstrata-strata-budget`。它使用现有的原生帧协议：绝对 `--manifest` 和 `--state` 路径，一次一帧 JSON。它不新增帧操作、权限，也不新增工作区以外的 crate。Windows 读数使用已固定的 `windows-sys` 0.61.2 特性 `Win32_System_ProcessStatus`，与 StrataPulse 已经链接的 crate 版本相同。

## 登记

只有该程序是宿主所给运行时同一目录中的普通文件时，`PluginHost::open` 才登记 `strata.budget`。条目是内置的，版本 1.0.0，并且禁用。它唯一的授权是 `storage.private`。它声明 `budget.regulate`，不声明事件。第三方 `install` 仍返回 `builtin_closed`。`uninstall` 仍返回 `builtin_retained`。没有该程序时，`enable` 仍返回 `implementation_absent`。

启用会启动调节进程。宿主不授予 `event.subscribe` 或 `event.publish`。插件帧的暂停和恢复保持进程。帧暂停返回 `action=keep`，不追加调节行，也不应用新限额。禁用和停止会回复，把字节 `joined` 写入 `released`，并以 0 退出。新的宿主从不暂停开始。暂停标志不写入 `host.json`。

## 限额

计划 007 的天花板保持不变。本计划不提高工作者、队列、订阅、准入或批量。可调整限额不能超过 1,000,000 CPU ppm、4,194,304 KiB 内存，或 65,536 个排队事件。这三个默认值等于天花板，因此普通进程保持运行。零是有效的收紧限额。

`budget.regulate` 的调用方可以发送 `cpu_ppm`、`mem_kib`、`queue`、`plugin` 和 `plugin_queue`。空载荷保持当前限额。`plugin=none` 清除那一个插件目标。`plugin=<id>` 配合 `plugin_queue=<n>` 保存一个插件的队列限额。`keep` 和 `none` 是哨兵，不是插件 id。调用方带来的测量键是 `invalid_input`。高于天花板的值是 `invalid_limit`。补零的数字会被拒绝。

## 压力

宿主用一份最多 1,024 字节的快照替换调用方载荷。它读取本进程，不采用调用方提供的数字。Linux 上驻留集是 `/proc/self/status` 的 `VmRSS`，CPU 时间是 `/proc/self/stat` 最后一个 `)` 之后的 `utime` 加 `stime`，再除以 `sysconf(_SC_CLK_TCK)`。Windows 上驻留集是 `GetProcessMemoryInfo` 的 `WorkingSetSize`，CPU 时间是 `GetProcessTimes` 的 100 纳秒单位。其他操作系统把这些读数报为 absent。缺席的读数不用 0 代替。

第一次 CPU 采样是基线，不触发暂停。之后的采样是经过的墙上时间的百万分比。测得值不截断到 1,000,000，因为可以有多个线程同时运行。限额天花板仍是 1,000,000。队列压力是已附着总线队列和已附着调度器队列在该来源存在时的和。缺失的总线或调度器保持 absent，并且不进入这个和。一个插件的限额是该插件的订阅积压，并且只在该插件正在运行时读取。

调节程序把快照和已保存的限额比较。队列压力优先，然后是内存，然后是 CPU，然后是那一个插件的积压。存在的读数高于限额就暂停。进入这次暂停时 `loss=1`。同一次暂停而总线丢失计数没有变化时 `loss=0`。调节程序仍处于暂停、总线丢失增加时，`loss` 再次为 1。每一个存在的读数都在限额内时，正在生效的暂停会恢复，否则动作是 hold。同一行追加到私有状态目录的 `regulation.jsonl`。达到 1 MiB 时，下一次追加之前把日志改名为 `regulation.prev.jsonl`，并只保留上一代。

丢失标记由宿主发布。插件不发布。标记是协议 `Record::Gap`，原因 `GapReason::Lost`，没有 range，设备是 `local-plugin-host`，提供者是 `strata.budget`，epoch 是宿主的 epoch，序列从 1 开始。重复暂停不再造一个洞。另一来源上较早的溢出缺口仍是 `Lost`。宿主在暂停时设置 `budget_paused`，在 hold 或恢复时清除它。该标志置位时，`emit` 在授权检查之前返回 `budget_paused`。`EventBus::publish` 不因此停止。恢复之后，这个插件的 `emit` 仍返回 `permission_denied`，因为清单不声明 `event.publish`。

## 本计划不做的事

磁盘 I/O、网络、存储和 GUI 刷新限额没有实现，也不报成零。这个缺口是 [OI-051](../records/OPEN_ITEMS.md#oi-051)。本计划不读取另一个进程的 CPU 或驻留集。服务和驱动快照、磁盘健康和界面页不在本计划中，见 [OI-047](../records/OPEN_ITEMS.md#oi-047)。文件和注册表采集器仍使用计划 009 的独立总线。本计划不附着那些总线。[OI-048](../records/OPEN_ITEMS.md#oi-048) 保持开放。持续压力仍属于计划 048。官方壳已经列出这个插件。本计划不新增页面、主题或视觉体系。[GUI_SOURCES.md](../records/GUI_SOURCES.md) 不变。首次启用仍属于计划 017。E2E-08 和 E2E-15 未完成。

## 验证

证据在[工程记录](../records/STATUS.md)。Linux 编写主机没有运行 Windows 测试。这个缺口是 [OI-050](../records/OPEN_ITEMS.md#oi-050)。它不关闭 [OI-001](../records/OPEN_ITEMS.md#oi-001)。
