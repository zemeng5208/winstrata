# 自身诊断

[English](../../architecture/HEALTH.md) · [文档索引](../INDEX.md) · [插件宿主](../../../core/src/plugins/mod.rs) · [计划 025](../roadmap/PLAN.md#pr-025)

计划 025 读取本产品自身的故障、队列、延迟和丢失。程序是 `winstrata-strata-health`。它使用现有的原生帧协议：绝对路径 `--manifest` 和 `--state`，一次一帧 JSON。它不新增帧操作、依赖，也不引入工作区以外的 crate。

## 登记

只有该程序是宿主所给运行时同目录下的普通文件时，`PluginHost::open` 才登记 `strata.health`。条目是内置的，版本 1.0.0，并且禁用。它只获得 `storage.private`。它声明 `health.observe`，不声明事件。第三方 `install` 仍返回 `builtin_closed`。`uninstall` 仍返回 `builtin_retained`。没有这个程序时，`enable` 仍返回 `implementation_absent`。

启用会启动诊断进程。宿主不授予 `event.subscribe` 或 `event.publish`，因此启用它不会增加总线订阅者，也不会发布观测。暂停和恢复保持进程。暂停停止向私有报告追加新行，但仍返回当前读数。禁用和停止先回复，再把字节 `joined` 写入 `released`，然后以 0 退出。没有需要汇合的采样线程。

## 读数

`health.observe` 的 `call` 忽略调用方载荷，改发宿主快照。快照在这次调用时读取。它包含：

| 字段 | 来源 |
|---|---|
| `plugin_faults` | 可用性为 `faulted` 或 `unavailable` 的已安装条目 |
| `bus_queued`、`bus_pending`、`bus_global`、`lost`、`filtered`、`queue_rejects` | 传给 `attach_bus` 的总线的 `BusStats`，否则 `bus=absent` |
| `sched_queued`、`sched_inflight`、`sched_rejected`、`sched_timed_out`、`sched_panicked` | 传给 `attach_scheduler` 的句柄的 `SchedulerStats`，否则 `sched=absent` |
| `sent_ns` | 生成快照时的宿主时钟 |

`Scheduler::handle` 只复制计数句柄。丢弃句柄不会关闭调度器，也不会取消任务。读取计数不改变队列准入、丢失或工作者行为。

进程拒绝缺少必填字段、重复字段、补零数字，或把 `absent` 和该来源的计数写在一起的快照。它不用 0 代替缺席的总线或调度器。报告的 `faults` 是插件故障，加上调度器存在时的超时和恐慌。报告的 `queues` 把存在的总线和调度器队列深度相加。报告的 `loss` 把总线丢失、过滤、队列拒绝和调度器拒绝相加，只计入存在的来源。`latency_ns` 是进程时钟减去 `sent_ns`；进程时钟更早时记 0。同一行追加到私有状态目录的 `diagnostics.jsonl`。到达 1 MiB 时，下一次追加前把日志改名为 `diagnostics.prev.jsonl`，只保留上一代。

## 本计划未完成的部分

动态限额、压力下的暂停和恢复，以及那种调节的丢失标记仍属于 StrataBudget。CPU、GPU、内存和硬件计数仍属于 StrataPulse。服务和驱动快照、混合存储容量、磁盘健康，以及这些读数的界面页都不在本计划中。文件和注册表采集器仍使用计划 009 创建的独立总线。本计划不附着那些总线，也不改变那些采集器发布的内容。官方壳已经列出这个插件。本计划不新增页面、主题或视觉体系。首次启用仍属于计划 017。E2E-08 和 E2E-15 未完成。

## 验证

证据在[工程记录](../records/STATUS.md)。本计划不关闭的缺口是 [OI-040](../records/OPEN_ITEMS.md#oi-040) 和 [OI-041](../records/OPEN_ITEMS.md#oi-041)。[OI-042](../records/OPEN_ITEMS.md#oi-042) 已在 Windows Server 2022 关闭。Windows 10 和 11 客户端仍是 [OI-001](../records/OPEN_ITEMS.md#oi-001)。
