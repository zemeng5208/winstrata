# 过滤插件

[English](../../architecture/FILTER.md) · [索引](../INDEX.md) · [插件宿主](PLUGINS.md) · [数据策略](DATA.md#section-8) · [脚本插件](SCRIPT.md) · [计划 028](../roadmap/PLAN.md#pr-028)

计划 028 是 `plugins/strata-filter` 中的官方插件 `strata.filter`。显示名称是 StrataFilter。程序是 `winstrata-strata-filter`。直接前置是计划 021、025 和 027。进程使用现有的长度前缀 JSON 帧。它不新增帧操作、权限名或工作区以外的 crate。

## 登记

只有传给宿主的运行时旁边存在普通文件 `winstrata-strata-filter` 时，`PluginHost::open` 才登记 `strata.filter`。种子项是内置、版本 1.0.0，并且在启用前保持禁用。能力是 `filter.apply`。权限是 `storage.private` 和 `plugin.call`。它不声明事件，因此不能发布一条自己随后又观察到的总线事件。第三方 `install` 仍返回 `builtin_closed`。`uninstall` 仍返回 `builtin_retained`。没有该程序时，`enable` 仍返回 `implementation_absent`。

启用会启动过滤进程。暂停保留进程，并且不把新项目放进缓冲。禁用和停止会回答，把字节 `joined` 写入 `released`，并以 0 退出。

## 基础保护

`protect` 丢掉每个 actor 为 `self` 的项目。这包括普通的内部写入、日志和 IPC，也包括随后对本插件自己的批量文件的观察。`tagged` 缺失或为 false 时，它也丢掉 `packet.raw`。actor 为 `other` 且 kind 为 `outside` 的项目由这层基础保护保留，包括对 WinStrata 设置或数据库路径的修改。`PluginHost::protect_observation` 调用同一函数，并且不启动进程。私有 `batches` 目录直到启用才创建。

脚本结论不能把已经丢掉的自身项目或未标记原始包放回批次。这些项目在脚本看到批次之前就被移除。

## GUI 规则和批量脚本

`filter.apply` 接受一个最多 1,024 字节且不含控制字符的 JSON 对象。`rule` 为 `process`、`app`、`path`、`event`、`protocol`、`ip`、`port` 或 `iface` 增加包含或排除。以 `/` 或 `\` 结尾的路径规则按前缀匹配。其他规则匹配整个字段。排除优先。某字段的包含列表非空时必须命中一项。`tune` 设置去重、连续非关键指标的聚合、1 到 32 的采样间隔、1 到 32 的缓冲、最长 60,000 毫秒的刷新间隔、压缩，以及归档 `keep` 或 `drop-metric`。`script` 保存一个 StrataScript 标识。空标识会清除它。

`apply` 暂存通过基础保护和 GUI 规则的项目。去重、采样、聚合和 `drop-metric` 发生在脚本之前。`flush`、缓冲已满或间隔到达时，对整个暂存批次请求一次脚本运行。宿主把该批次写入脚本私有目录的 `batch.txt`，并调用一次 `script.workbench`。脚本为每一行打印 `keep` 或 `drop`。故障、缺失的脚本、长度不一致，或脚本插件没有在运行，都会写入 `fallback.log`，然后改写基础规则的结果。脚本仍然不能恢复基础保护已经丢掉的项目。

## 批量写入和 I/O

保留的行在每次刷新时写成一个 `batches/bNNNN.wfb` 文件。文件以 `WFB1` 开头。连续相同的行在这种形式比原始行更小时按计数存放。结果报告 `seen`、`kept`、`suppressed`、`excluded`、`sampled`、`deduped`、`aggregated`、`archived`、`scripted`、`avoided`、`overhead`、`raw` 和 `stored`。`avoided` 是没有保留的事件。`overhead` 是批量文件的字节数。同样的数字写在 `stats.json`。

## 官方页面

官方页面组合现有的 shadcn-admin [Button](../../../packages/ui/vendor/shadcn-admin/src/components/ui/button.tsx)、[Input](../../../packages/ui/vendor/shadcn-admin/src/components/ui/input.tsx) 和 [Badge](../../../packages/ui/vendor/shadcn-admin/src/components/ui/badge.tsx)。它不加入 Fluent UI、Astryx 或新的视觉体系。它不发送 `enable` 或 `grant`。桥命令 `filter-apply` 只在插件已经运行时转发载荷。尚未选择存储位置时，桥返回的 kept、avoided 和 overhead 都是 0，并且不创建插件目录。

## 本计划之外

计划 029、030 和 038 不在本计划内。标记持久化、删除和快照比较仍留在那些计划。磁盘、网络和存储预算仍是 [OI-051](../records/OPEN_ITEMS.md#oi-051)。头 `bc009ea2ee3481ae8a9fea295c246380249cd32a` 上的 Windows Server 2022 工作区作业通过了过滤测试。这关闭该头上的 [OI-052](../records/OPEN_ITEMS.md#oi-052)。官方过滤页没有在 Windows 上打开。那是 [OI-053](../records/OPEN_ITEMS.md#oi-053)。[E2E-04](../roadmap/ACCEPTANCE.md#e2e-04)、[E2E-05](../roadmap/ACCEPTANCE.md#e2e-05)、[E2E-07](../roadmap/ACCEPTANCE.md#e2e-07) 和 [E2E-08](../roadmap/ACCEPTANCE.md#e2e-08) 仍待完成。
