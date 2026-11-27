# 脉冲计数器

[English](../../architecture/PULSE.md) · [采集进程](../../../plugins/strata-pulse/src/main.rs) · [宿主](../../../core/src/plugins/mod.rs) · [计划 024](../roadmap/PLAN.md#pr-024)

计划 024 在 `plugins/strata-pulse` 中运行 StrataPulse。程序名是 `winstrata-strata-pulse`。它使用现有的原生帧协议：绝对路径 `--manifest` 和 `--state`，一次一帧 JSON。它不新增帧操作、协议权限或依赖。

直接前提 010、012、014 和 016 已在 main。本计划不实现 020、021、022、023、025、026、027 或 032。

## 登记

<a id="plugin"></a>

只有 `winstrata-strata-pulse` 是宿主收到的运行时同目录中的普通文件时，`PluginHost::open` 才登记 `strata.pulse`。条目是内置插件，版本 1.0.0，并且禁用。授权是 `event.publish` 和 `storage.private`。第三方 `install` 仍返回 `builtin_closed`。`uninstall` 仍返回 `builtin_retained`。没有这个相邻文件时，`enable` 仍返回 `implementation_absent`。

`strata.process` 仍使用自己的相邻文件检查。`strata.registry` 仍在进程内运行。启用脉冲不会启动这些采集器，它们的采样路径也不变。

## 采样

采集进程在 `ws-pulse` 线程上大约每 200 毫秒采样一次。一条读数是 `measured`、`baseline`、`unsupported` 或 `failed` 之一。

测得的读数带有整数值和来源。基线表示计数器已经出现一次，但还没有差分。不支持表示这台主机没有该来源，或这个采集器没有该 API 的读取器。失败表示来源存在，但读取或计算没有得到值。基线、不支持和失败都没有数值。缺失的计数器不记成 0。

CPU 利用率是忙时计数差分除以总计数差分，单位是百万分之一。忙时不包含 idle 和 iowait。第一次采样是基线。内存在 Linux 上来自 `MemAvailable`，在 Windows 上来自 `GlobalMemoryStatusEx`。只有两个值都读到，且可用内存不超过总量时，已用内存才是总量减去可用量。`SwapTotal` 存在时记录交换区总量，包括真实的 0。`/proc/loadavg` 存在时，负载是一分钟平均负载的毫单位。

频率、CPU 温度、hwmon、电池容量、电池功率和 RAPL 能量在对应 sysfs 文件存在时读取。频率文件里的 0 不当作测量。GPU 忙碌百分比和显存来自存在的 DRM sysfs。报告 0% 的空闲 GPU 是测得值。缺失的 DRM 计数器是不支持，原因是 `no_counter`。

硬件性能计数器通过 `perf_event_open` 读取 cycles 和 instructions。采集器先尝试每个在线 CPU，再尝试本进程的一个计数器。拒绝访问是 `perf_denied`。其他打开失败是 `perf_unavailable`。第一次成功读取是基线，之后保存差分。只打开部分 CPU 时来源是 `perf.cpu.partial`，并记录打开的 CPU 数。

采样文档保留进程数、RSS 最高的八个进程，以及 CPU 时间差分最高的八个进程。逐进程行不是指标序列。进程 CPU 的第一次采样是基线。读不到 RSS 的进程不进入 RSS 列表，并且该采样标为 limited。这个采集器不发布 `process.started` 或其他进程生命周期事件。

Windows 用 `GetSystemTimes`、`GlobalMemoryStatusEx`、`CallNtPowerInformation`、`GetSystemPowerStatus`，以及 Toolhelp、`GetProcessTimes` 和 `GetProcessMemoryInfo` 填写同一份文档。负载、交换区、GPU、显存、温度、RAPL 和 perf 保持不支持，原因是 `reader_absent`、`no_loadavg` 或 `no_swap`。`reader_absent` 表示这个采集器没有调用该 API，不是设备报告计数器不存在。电池百分比 255 和无电池标志保持不支持，并且没有数值。[OI-040](../records/OPEN_ITEMS.md#oi-040) 追踪 Windows 的 GPU、显存和温度读取器。

驱动 SMART、逻辑和物理写入以及磨损仍属于计划 032。这个采集器不读取它们，也不编造它们。

## 历史

每次保留的采样都往私有状态目录的 `history.jsonl` 追加一个 JSON 对象，并替换 `latest.json`。达到 16 MiB 时，下一次追加前把日志改名为 `history.prev.jsonl`，只保留上一代。`peaks.json` 保存每个序列测得值的最大值。不支持和基线不推动峰值。`samples` 是已保留的采样次数。暂停停止新采样并保持进程运行。禁用和停止会等待 `ws-pulse` 线程结束，把 `joined` 写入 `released`，并以 0 退出。

测得的值还通过 `Core::commit_plugin_metrics` 提交到计划 012 的指标存储。Windows 上会先去掉规范化路径的 `\\?\` 前缀，再打开工作区锚点。序列 id 是 `pulse.` 加读数 id，例如 `pulse.memory.total.bytes` 和 `pulse.cpu.usage.ppm`。状态目录必须是 `<用户>/plugins/state/strata.pulse`。存在 `vault.wsv` 时提交被拒绝，采样把 `store` 记为 `locked`，私有历史仍然写入。没有测得值的采样记为 `private_only`，并且不打开存储。这些观察不通过 `emit` 发布。对 `pulse.observe` 的 `invoke` 返回最近一次采样数、测得数和不支持数。

官方外壳已经列出这个插件。本计划不新增页面、图表、主题或视觉体系。[GUI_SOURCES.md](../records/GUI_SOURCES.md) 不变。首次启用仍属于计划 017。StrataHealth 和 StrataBudget 仍属于计划 025 和 026。

## 验证

在这台 Linux x86_64 主机上，脉冲测试读取了 `/proc/stat`、`/proc/meminfo` 和 `/proc/loadavg`。这台主机没有 DRM GPU 计数器、cpufreq、温度区、hwmon、电池或 RAPL 目录，因此这些读数是不支持。观测测试要求测得的内存总量、不超过总量的可用内存、等于二者之差的已用内存、0 到 1,000,000 ppm 的 CPU 差分、至少为 1 的进程数、首行历史里没有数值的 CPU 基线、内存总量峰值、已提交的混合序列 `pulse.memory.total.bytes`，以及不支持的 GPU 读数没有对应的混合序列。暂停期间采样计数不变。禁用写入了 `joined`。工作区运行的计数见[工程记录](../records/STATUS.md)。

这台主机没有运行 Windows 测试。E2E-08 未完成。
