# 注册表插件

[English](../../architecture/REGISTRY.md) · [插件宿主](PLUGINS.md) · [计划 009 监视](FILE-REGISTRY.md) · [计划 019](../roadmap/PLAN.md#pr-019)

计划 019 是 `strata.registry`。它在插件宿主进程内监视当前用户和本机注册表值，并把前后证据发布到现有事件总线。它不加载 `strata-reg.sys`。它不替换计划 009 的监视。计划 009 仍然观察一个选定目录和一个 `HKEY_CURRENT_USER` 子键。环境变量和 PATH 仍属于计划 020。文件插件仍属于计划 021。启动项 Run 键不是这个插件的用途。

## 进程内宿主

原生帧协议放不下前后值，子进程也不能主动推送事件。因此监视留在 `PluginHost` 内。启用 `strata.registry` 不写入已安装包，也不启动 `winstrata-runtime`。`load`、`update`、`pin`、`grant`、`call` 和 `ping` 返回 `builtin_closed`。`uninstall` 仍返回 `builtin_retained`，与其他官方身份相同。其余 24 个官方身份仍报告 `implementation_absent`。

`host.json` 在已安装条目旁边保存 `registry_enabled`、`registry_availability` 和 `registry_scope`。缺少这些字段时读成未启用、`ready` 和空范围。空范围表示产品默认范围。保存的范围无效时，打开失败，错误是 `invalid_record`。重新打开已启用的插件不会启动监视。下一次 `start` 或 `enable_batch` 才会启动，并且只在保存的可用性是 `ready` 时启动。批量启用不会重试已经保存的 `unsupported`。

`enable` 先把插件记为已启用，然后启动监视。Linux 上启动返回 `unsupported`，不发布事件，报告保持已启用和 `unavailable`。`disable` 停止线程，清除启用状态，并把可用性恢复为 `ready`。`stop` 保持插件已启用。监视停止期间发生的变化不会发布，下一次启动把它当作新基线。`pause` 保留线程。这次暂停期间的变化为该键和该暂停代发布一条 `paused` 缺口，然后推进基线，不发布逐值差异。`resume` 继续。

报告种类是 `builtin`，版本是 0.1.0，进程号为空，授权是 `event.publish`，允许批量启动。阶段是 `disabled`、`stopped`、`running`、`paused` 或 `unavailable`。

## 范围

监视最多持有四个键。集合必须同时包含至少一个 `HKCU` 键和一个 `HKLM` 键。配置单元文本只能是 `HKCU` 或 `HKLM`。其他用户的 `HKU` 配置单元不在本计划内。子键沿用计划 009 的规则：1 到 8 段，最多 200 字节，不能有空段、`.`、`..`、`/`、`:` 或控制字符，也不能以反斜杠开头或结尾。重复键按不区分大小写比较。被拒绝的路径不会复制进错误。说明固定为 `invalid registry scope`。

产品默认范围是 `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Shell Folders` 和 `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion`。监视运行时修改范围会以 `running` 拒绝。下一次启动使用已保存的范围。

现有插件页通过宿主报告列出这个可用性。本计划不增加页面、查询命令或视觉体系。GUI 查询仍属于 E2E-02，不是这次验收。

## 事件

设备是 `strata.registry`。提供者是 `windows.reg_notify`。纪元是 `reg-N`，每次尝试启动都增加。序号从 1 开始，观察和缺口共用。省略 `precision_ns`。省略进程字段。这些 API 不报告变更时刻的精度，也不报告执行进程。当前进程身份能放入标识符时，用户使用该身份。否则省略用户。

主体是 `HKCU\{subkey}\{name}` 或 `HKLM\{subkey}\{name}`。证据语法与计划 009 相同：新增是 `t={type};a={hex}`，删除是 `t={type};b={hex}`，类型不变的修改是 `t={type};b={before};a={after}`，类型变化使用 `tb`、`ta`、`b` 和 `a`。十六进制为小写。证据必须能放入标识符，不截断。放不下时，证据改为提供者名称，部分原因为 `value_exceeds_identifier`。

每个键以 `KEY_QUERY_VALUE | KEY_NOTIFY | KEY_WOW64_64KEY` 打开。`RegNotifyChangeKeyValue` 是异步的，也不递归。一个线程等待停止事件和仍存活的通知事件。它先重新武装，再读取下一次快照。快照在超过 64 个值、名称或值需要超过 256 个 UTF-16 字符或 1024 字节、名称不是 UTF-16，或名称重复时不完整。不完整的快照发布一条 `registry.key_changed`，部分原因是 `snapshot_incomplete`，并且不编造值差异。空差异不发布。启动时已经存在的基线不会发布。如果通知在该基线和第一次武装之间已经触发，线程发布一条 `lost` 缺口并重新快照，然后 `start` 才返回。硬失败发布 `source_failure` 并放弃该键。如果每个键都失败，线程退出。查询环保留最近 128 条事件。

## 仍保留的限制

没有加载已签名的注册表回调。没有在 Secure Boot 和 HVCI 下验证。本变更没有测试 Windows 10 和 Windows 11 客户端。Windows 测试针对 `x86_64-pc-windows-msvc` 编译，并在 Windows 工作区作业中运行。该作业是 Windows Server 2022。Linux 编写主机打不开真实注册表。E2E-02 仍要求 GUI 查询、文件、PATH 和其他用户。计划 020 和计划 021 没有开始。
