# 环境插件

[English](../../architecture/ENV.md) · [插件宿主](PLUGINS.md) · [计划 020](../roadmap/PLAN.md#pr-020)

计划 020 是 `strata.env`。它在插件宿主进程内监视当前用户环境存储和系统环境存储，并把前后证据发布到现有事件总线。它不加载驱动。它不替换计划 009。它不监视计划 019 的注册表范围。启动项 Run 键、文件、服务和任务不是这个插件。

## 进程内宿主

原生帧协议放不下前后值，子进程也不能主动推送事件。监视留在 `PluginHost` 内。启用 `strata.env` 不写入已安装包，也不启动 `winstrata-runtime`。`load`、`update`、`pin`、`grant`、`call` 和 `ping` 返回 `builtin_closed`。`uninstall` 返回 `builtin_retained`。注册表插件仍是计划 019 的监视。其余仍没有采集器的官方身份保持 `implementation_absent`。

`host.json` 在已安装条目旁边保存 `env_enabled`、`env_availability` 和 `env_scope`。缺少这些字段时读成未启用、`ready` 和空范围。空范围表示产品默认范围。保存的范围无效时，打开失败，错误是 `invalid_record`。重新打开已启用的插件不会启动监视。下一次 `start` 或 `enable_batch` 才会启动，并且只在保存的可用性是 `ready` 时启动。批量启用不会重试已经保存的 `unsupported`。

`enable` 先把插件记为已启用，然后启动监视。Linux 上启动返回 `unsupported`，不发布事件，报告保持已启用和 `unavailable`。`disable` 停止线程，清除启用状态，并把可用性恢复为 `ready`。`stop` 保持插件已启用。监视停止期间发生的变化不会发布，下一次启动采用新基线。`pause` 保留线程。这次暂停期间的变化为该存储和该暂停代发布一条 `paused` 缺口，然后推进基线，不发布逐值差异。`resume` 继续。

报告种类是 `builtin`，版本是 0.1.0，进程号为空，授权是 `event.publish`，允许批量启动。阶段是 `disabled`、`stopped`、`running`、`paused` 或 `unavailable`。

## 范围

范围恰好是两个存储。一个必须是 `HKCU`，一个必须是 `HKLM`。配置单元文本只能是 `HKCU` 或 `HKLM`。子键沿用计划 009 的规则：1 到 8 段，最多 200 字节，不能有空段、`.`、`..`、`/`、`:` 或控制字符，也不能以反斜杠开头或结尾。重复键按不区分大小写比较。被拒绝的路径不会复制进错误。说明固定为 `invalid env scope`。

产品默认范围是 `HKCU\Environment` 和 `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment`。监视运行时修改范围会以 `running` 拒绝。下一次启动使用已保存的范围。

现有插件页通过宿主报告列出这个可用性。本计划不增加页面、查询命令或视觉体系。它不加入 Fluent UI 或 Astryx。GUI 查询仍属于 E2E-02，不是这次验收。

## 事件

设备是 `strata.env`。提供者是 `windows.env_notify`。纪元是 `env-N`，每次尝试启动都增加。序号从 1 开始，观察和缺口共用。省略 `precision_ns`。省略进程字段。当前进程身份能放入标识符时，用户使用该身份。否则省略用户。

主体是 `user\{name}` 或 `system\{name}`，对象种类是 `environment`。这段文字放不进标识符时，省略主体，事件为部分事件，原因是 `identifier_limit`。名称不会被复制进事件。名称比较不区分 ASCII 大小写。`REG_SZ` 和 `REG_EXPAND_SZ` 是文本，按 UTF-16 解码，并在证据中保存 UTF-8 字符串的小写十六进制。标记是 `sz` 和 `expand`。新增是 `t={token};a={hex}`。删除是 `t={token};b={hex}`。同类型修改是 `t={token};b={before};a={after}`。类型变化使用 `tb`、`ta`、`b` 和 `a`。证据必须能放入标识符，不截断。放不下时，非 PATH 值使用证据 `windows.env_notify`，部分原因是 `value_exceeds_identifier`。

变量名在任意 ASCII 大小写下为 `PATH` 时，使用 `env.path_added`、`env.path_removed` 或 `env.path_changed`。其他变量使用 `env.variable_added`、`env.variable_removed` 或 `env.variable_changed`。完整 PATH 证据放不下时，监视器不发出截断后的完整值。它按分号拆分，丢掉空段，并对每个自身证据放得下的新增段发出一条完整的 `env.path_added` 事件，对每个自身证据放得下的删除段发出一条完整的 `env.path_removed` 事件。放不下的段，或没有段差异的变化，例如只改变顺序，发出一条部分 `env.path_changed`，原因是 `value_exceeds_identifier`。非文本值发出一条部分事件，原因是 `value_not_text`，并且不包含字节。

每个存储以 `KEY_QUERY_VALUE | KEY_NOTIFY | KEY_WOW64_64KEY` 打开。`RegNotifyChangeKeyValue` 是异步的，也不递归。一个线程等待停止事件和仍存活的通知事件。它先重新武装，再读取下一次快照。快照在达到 256 个值、名称需要超过 256 个 UTF-16 码元、值需要超过 32768 字节、名称不是 UTF-16，或名称忽略大小写后重复时不完整。不完整的快照发布一条 `env.block_changed`，部分原因是 `snapshot_incomplete`，并且不编造值差异。空差异不发布。启动时已经存在的基线不会发布。如果通知在该基线和第一次武装之间已经触发，线程发布一条 `lost` 缺口并重新快照，然后 `start` 才返回。硬失败发布 `source_failure` 并放弃该存储。如果每个存储都失败，线程退出。查询环保留最近 128 条事件。

## 仍保留的限制

本变更没有测试 Windows 10 或 Windows 11 客户端。Windows 工作区作业是 Windows Server 2022。在送审头 `579f85d63b338e4502f95abdbd122c3a815e2902` 上，该作业通过了 `environment_is_in_process_and_linux_does_not_publish` 和四项易失键测试。Linux 编写主机打不开真实的用户环境存储或系统环境存储。E2E-02 仍要求 GUI 查询、文件和其他用户。计划 021 没有开始。Windows 宿主测试在真实的用户和系统存储上写入一个探测变量并追加一段 PATH，然后恢复原来的字节。插件包里的 Windows 测试使用易失键，不写入这两个存储。
