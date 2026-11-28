# 存储插件

[English](../../architecture/STORAGE.md) · [索引](../INDEX.md) · [插件宿主](../../../core/src/plugins/mod.rs) · [计划 030](../roadmap/PLAN.md#pr-030)

计划 030 是 `plugins/strata-storage` 中的官方插件 `strata.storage`。显示名是 StrataStorage。程序是 `winstrata-strata-storage`。它使用现有的原生帧协议：绝对的 `--manifest` 和 `--state` 路径，以及一次一个 JSON 帧。它不新增帧操作、权限，也不引入工作区以外的 crate。删除留在 Core。

## 登记

只有当 `winstrata-strata-storage` 是传给 `PluginHost::open` 的运行时旁边的普通文件时，宿主才登记 `strata.storage`。登记的条目是内置插件，版本 1.0.0，在启用前保持禁用。能力是 `storage.configure`、`storage.decline`、`storage.propose` 和 `storage.status`。权限是 `storage.private` 和 `plugin.call`。它不声明事件。第三方 `install` 仍返回 `builtin_closed`。`uninstall` 仍返回 `builtin_retained`。没有该程序时，`enable` 仍返回 `implementation_absent`。

启用会启动存储进程。暂停和恢复保留该进程。禁用和停止先回答，把字节 `joined` 写入 `released`，再以 0 退出。载荷里 `confirm` 为真，或 `op` 为 `confirm` 时，返回 `confirmation_required`，并且不写任何文件。

## 阈值

Core 把阶梯保存在 `<user>/storage-alerts.json`，与 `core.json` 并列。插件进程只写自己的私有文件 `policy.json`、`decline.json` 和 `proposal-request.json`。`storage_notice` 在 `policy.json` 已经存在时采用其中的阈值，然后消费一张 `decline.json`。`Core::configure_storage` 写入同一条阶梯。如果 `policy.json` 已经存在，它会覆盖该文件，使两份副本不再分叉。`Core::decline_storage` 只移动一步，并且不读取那张票据。

候选默认值使用十进制字节：已持久化的捕获在 1,000,000,000 时警告，之后每 1,000,000,000 再警告一次；已保存日志在 2,000,000,000 时警告，之后每 500,000,000 再警告一次。配置值是 1 到 1,000,000,000,000,000 的整数。这些单位不是所有者决定。[OI-024](../records/OPEN_ITEMS.md#oi-024) 保持开放。

已保存日志是 `<user>/logs` 下的普通文件，名称匹配 `^[a-z][a-z0-9._-]{0,47}$` 并且以 `.log` 结尾。事件、指标、索引、备份和结构化记录不计入。捕获是 `<user>/data/captures` 下普通文件的字节。警告状态是已武装阈值 `first + declines * repeat`。用量达到或超过该阈值时为等待中。一次拒绝增加一步。如果用量仍然达到或超过新的阈值，警告保持等待。没有回应不会删除，也不会增加拒绝次数。清理把拒绝次数设回 0，再按首次阈值重新计算。改变阈值也按同一方式重算。

## 确认与索引

`propose_storage` 解析范围并保存提案编号。它不删除。请求里的 `confirm` 为真时返回 `confirmation_required`，并且不保存提案。`expiration` 返回 `expiration_unconfirmed`，并且不删除。`capacity` 返回 `selection_required`，并且不挑选分段。`confirm_storage` 只有在受信任调用方传入 `confirm` 为真，并且提案编号仍然匹配从当前索引再次解析出的范围时才删除。不匹配时返回 `stale`，并且不删除。

只有当分段里的每条记录都匹配范围，并且没有一条受保护时，该分段才会被删除。否则它以 `protected` 或 `partial` 被保留。删除是一条日志提交：引用和水标为空，尾部带分段对，以及结构化分段里的对象编号。没有删除的提交省略尾部，因此旧日志仍然可以解码。重放只在同一已应用前缀里的后续提交删除该分段时，才跳过缺失的载荷，然后把该分段的水标设为 0 并忘掉它的索引项。存活分段里指向已删除对象的关系会被移除。类别 `indexes` 和分段 0 不能删除。日志同步之后，宿主删除分段文件。审计行追加到 `<user>/storage-audit.jsonl`。

清理不跟随五类数据分段和 `<user>/logs` 下普通文件以外的路径。存储不可用时返回 `storage` `unavailable`，并且不删除。插件进程被禁用时，Core 仍然返回这条阶梯。

## 脚本与页面

工作台操作 `storage` 不启动解释器。已启用且拥有该脚本自己的 `plugin.call` 授权时，`configure`、`decline`、`propose` 和 `status` 返回 `code` `storage_request`。宿主把该请求转给存储进程。`confirm`，或 `confirm` 为真，返回 `confirmation_required`，并且不转发。没有该授权时，工作台返回 `permission_denied`，并且不写文件。脚本插件清单不获得 `plugin.call`。

官方页面 `/storage` 组合现有的 shadcn-admin [Button](../../../packages/ui/vendor/shadcn-admin/src/components/ui/button.tsx)、[Input](../../../packages/ui/vendor/shadcn-admin/src/components/ui/input.tsx) 和 [Badge](../../../packages/ui/vendor/shadcn-admin/src/components/ui/badge.tsx)。它不引入 Fluent UI、Astryx 或新的视觉系统。它不发送 `enable` 或 `grant`。桥接命令 `storage-status`、`storage-configure`、`storage-decline`、`storage-propose` 和 `storage-confirm` 调用 Core。它们不启用插件。尚未选择存储位置时，`storage-status` 返回 `enabled` 为假且 `storage` 为 `unavailable`，并且不创建插件目录。删除使用 `storage-confirm` 上的并列字段 `confirm` 为真。托盘提示仍是产品名。该缺口是 [OI-057](../records/OPEN_ITEMS.md#oi-057)。这台主机没有运行 Windows 测试和 Windows 页面会话。那些缺口是 [OI-058](../records/OPEN_ITEMS.md#oi-058) 和 [OI-059](../records/OPEN_ITEMS.md#oi-059)。MCP 配置属于计划 045，即 [OI-060](../records/OPEN_ITEMS.md#oi-060)。

## 本计划不做的事

本计划不关闭 [OI-024](../records/OPEN_ITEMS.md#oi-024)、[OI-047](../records/OPEN_ITEMS.md#oi-047) 或 [OI-051](../records/OPEN_ITEMS.md#oi-051)。它不实现计划 031 或更后的计划。[E2E-05](../roadmap/ACCEPTANCE.md#e2e-05) 和 [E2E-15](../roadmap/ACCEPTANCE.md#e2e-15) 保持开放。磁盘、网络、存储和 GUI 刷新预算仍属于计划 026。

## 验证

证据在[工程记录](../records/STATUS.md)。
