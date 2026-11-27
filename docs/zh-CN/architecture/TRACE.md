# 跟踪插件

[English](../../architecture/TRACE.md) · [插件宿主](PLUGINS.md) · [计划 009 监视](FILE-REGISTRY.md) · [计划 021](../roadmap/PLAN.md#pr-021)

计划 021 是 `strata.trace`（StrataTrace），即文件插件。它在 `PluginHost` 内监视目录，并把用户态文件元数据发布到现有事件总线。宿主方式与 `strata.registry` 相同。它没有包，也不启动 `winstrata-runtime`。它不加载 `strata-fs.sys`。它不替换计划 009 的单目录监视。环境变量和 PATH 仍属于计划 020。内容哈希和签名仍属于计划 033。计划 019 的用户态注册表证据不是内核归因。这里的文件证据只是用户态元数据。

## 进程内宿主

`load`、`update`、`pin`、`grant`、`call` 和 `ping` 返回 `builtin_closed`。`uninstall` 返回 `builtin_retained`。版本是 0.1.0。授权是 `event.publish`。允许批量启动。

`host.json` 保存 `trace_enabled`、`trace_availability` 和 `trace_scope`。空范围不代表产品默认磁盘。Linux 上启动返回 `unsupported`，不发布事件，报告保持已启用和 `unavailable`。`enable_batch` 不重试已经保存的 `unsupported`。`disable` 把可用性恢复为 `ready`。

现有插件页通过宿主报告列出这个可用性。本计划不增加页面，也不增加 GUI 来源行。不加入 Fluent UI、Astryx 或新的视觉体系。官方外壳已经根据宿主报告列出插件可用性。

## 范围

`set_trace_scope` 接受 1 到 4 个已经存在的目录。它拒绝相对路径、`.` 和 `..`、控制字符、符号链接、非目录、重复项，以及其他用户的私有配置目录。被拒绝的路径不会复制进错误。说明固定为 `invalid trace scope`。监视运行时不能修改范围。

## Windows 监视

Windows 监视使用 `ReadDirectoryChangesW`。监视不递归。缓冲区为 16 KiB。提供者是 `windows.read_directory_changes`。设备是 `strata.trace`。纪元是 `fs-N`。从不读取文件字节。没有内容版本，也没有内容哈希。

元数据证据是 `sb={before-size-or-dash};sa={after-size-or-dash};pd={0-or-1}`。没有大小时用连字符。`pd` 是权限变化标志，取 0 或 1。前后所有者都存在且不同，并且加长后的字符串仍能放入标识符时，证据附上 `;ob={before-owner};oa={after-owner}`。SDDL 放得下时附上 `;sd={sddl}`。这两个字段都不截断。SDDL 放不下时，证据保留大小这一对，部分原因是 `permissions_exceed_identifier`。大小这一对本身放不下时，证据改成提供者名称，部分原因是 `metadata_exceeds_identifier`。删除不为当前所有者编造值。

省略 `precision_ns`。省略进程字段。这些 API 不报告变更时刻的精度，也不报告执行进程。这不是内核归因。

## 事件

事件类型沿用计划 009：`file.added`、`file.modified`、`directory.added`、`directory.modified`、`path.removed`、`path.renamed_from`、`file.renamed_to` 和 `directory.renamed_to`。快照不完整时发布 `directory.changed`。部分原因是 `path_escapes_root`、`reparse_unresolved`、`metadata_unavailable`、`rename_unpaired`、`name_not_unicode_*`、`snapshot_incomplete`、`security_unavailable`、`permissions_unavailable`、`owner_unavailable`、`absolute_path_exceeds_identifier`、`path_exceeds_identifier`、`permissions_exceed_identifier` 和 `metadata_exceeds_identifier`。启动时已经存在的基线不会发布。16 KiB 溢出造成的丢失是一条 `lost` 缺口，不按文件归因。暂停为每个目录、每一暂停代发布一条 `paused` 缺口，并推进基线，不发布逐文件差异。监视环保留最近 128 条事件。

## 历史

`trace_query` 返回该监视环。`trace_history` 返回主体等于该精确路径的实时事件。计划 017 的基线条目等于该路径时，再返回投影 `trace.reconstructed`，证据是 `state=reconstructed`。其他来源设备的主体或证据等于该精确路径时，每个这样的设备返回一条 `trace.path_shared`。该证据是 `with={device}`。投影的提供者是 `winstrata.trace`。路径相等不是因果断言，也不是时间相关。仅有时间重叠不会建立关联。投影省略进程字段和 `precision_ns`。

## 仍保留的限制

没有加载已签名的文件微过滤驱动。编写主机是 Linux。这台主机上的 Linux 语法测试已通过。在 Windows Server 2022 上，头 `dc9d9e378f9f585929f6acfc18944411f1bbcc75` 的两项通过不能关闭换基之后的 [ERR-019](../records/ERRORS.md#err-019)。换基后的头 `2b0ad8d871dad905ea7fae551a2f9f69bb3b8386` 通过了 `windows_trace_records_metadata_without_content` 和 `windows_file_watch_overflow_records_a_lost_gap`。这次元数据通过解决 ERR-019。溢出通过不重新打开 OI-018。没有运行 GUI 会话。计数写在[工程记录](../records/STATUS.md)。E2E-02 和 E2E-03 未完成。OI-016 和 OI-018 已关闭。文件证据不是内核归因。计划 020 已在 main 上，本计划不重新实现它。
