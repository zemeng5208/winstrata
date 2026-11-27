# 启动插件

[English](../../architecture/STARTUP.md) · [插件宿主](PLUGINS.md) · [服务插件](SERVICES.md) · [注册表插件](REGISTRY.md) · [计划 023](../roadmap/PLAN.md#pr-023)

计划 023 是 `plugins/strata-startup`。程序名是 `winstrata-strata-startup`。插件 id 是 `strata.startup`。它复用原生帧协议，不增加帧操作，也不增加新的外部 crate。`windows-sys` 0.61.2 已经获批，并且只在 Windows 上使用。`libc` 是已有的 Unix 依赖，只用来读取 passwd 家目录。

## 登记

只有当 `winstrata-strata-startup` 是运行时旁边的普通文件时，`PluginHost` 才登记 `strata.startup`。这条记录是内置插件，版本 1.0.0，启用前保持禁用。授予的权限是 `event.publish` 和 `storage.private`。`uninstall` 保持 `builtin_retained`。清单依赖为空。计划 019 和计划 022 是合并顺序上的前置，不是运行时依赖。`strata.services`、`strata.process` 和 `strata.registry` 的采集不变。Run 键在这里读取。计划 019 不监视它们。

采集器只读。它不创建、不修改、也不删除 Run 值、启动快捷方式、cron 文件、timer 或计划任务。宿主作业只允许一个进程，因此采集器不启动 `schtasks` 或 `systemctl`。

官方外壳已经列出这个插件。本计划不增加页面、主题或视觉系统。[GUI_SOURCES.md](../../records/GUI_SOURCES.md) 不变。

## 采集器

事件写入私有 `events.jsonl`，不用 `emit` 发布。文件在 16 MiB 时轮转到 `events.prev.jsonl`。调用 `startup.observe` 返回采样次数和事件条数。采样线程两次采样之间等待 200 ms。暂停停止采样。禁用和停止会汇合线程，把字节 `joined` 写入 `released`，并以退出码 0 结束。没有存活到下一次采样的变化不会被编造出来。

每次成功采样之后，采集器写入 `inventory.json`。文件是版本 1 的 JSON：设备 id 和事实。下一个进程恢复这个文件，因此采集器停止期间发生的变化发出带 `before=` 的 `startup.changed` 或 `task.changed`，而不会再发一次基线。缺少文件时开始一条基线。空文件或无法读取的文件先发出一条 `SourceFailure` 缺口，然后开始新的基线，并且不编造删除。导出失败不会删掉上一份清单。`state.json` 记录启动项数量和任务数量。`read_state` 仍然拒绝超过 1024 字节或含有控制字符的文件，因此历史文件是 `inventory.json`，不是新的帧操作。

## 来源

设备是 `strata.startup`。快照证据为 `windows` 时，提供方是 `windows.startup`，否则是 `posix.startup`。覆盖范围是 `Windows` 或 `HostFiles`。省略 `precision_ns`、用户和进程。主体种类是 `startup` 或 `task`。证据是一个 `Id`：1 到 256 字节，两端没有空白，也没有控制字符。放不下的字段被丢掉，事件标记为部分。部分原因是 `principal-exceeds-identifier`、`location-exceeds-identifier`、`command-exceeds-identifier`、`schedule-exceeds-identifier` 和 `before-exceeds-identifier`，以及事实原因 `command-absent`、`link-unresolved`、`approved-unreadable`、`extra-actions` 和 `task-unreadable`。

Windows 只读、不写：

- `HKCU` 与 `HKLM` 的 `Software\Microsoft\Windows\CurrentVersion\Run` 和 `RunOnce`
- `HKLM` 的 `Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run` 和 `RunOnce`
- `HKCU` 与 `HKLM` 的 `Software\Microsoft\Windows\CurrentVersion\Policies\Explorer\Run`
- Run、Run32 和 StartupFolder 的 `StartupApproved`。缺失、空白或低位为 0 的值视为启用。首字节低位为 0 时启用（`0x02` 和 `0x06`）。低位为 1 时禁用（`0x03` 和 `0x01`）。这是观察到的任务管理器变化，不是 Microsoft 规范。读不到 `StartupApproved` 时，该项保持启用，并标记 `approved-unreadable`。
- 用户启动文件夹 `{B97D20BB-F46A-4C97-BA10-5E3608430854}` 和公共启动文件夹 `{82A5EA35-D9CD-47C5-9629-E15D2F714E6E}`，通过 `SHGetKnownFolderPath` 和 `KF_FLAG_DONT_VERIFY`（`0x4000`）读取
- `%SystemRoot%\System32\Tasks` 下的任务计划程序 XML。编码是带 BOM 的 UTF-16 LE，或开头为 `<` 的 UTF-16。任务开关是 `Settings.Enabled`。第一个触发器的 `Enabled` 不是这个开关。命令是动作的 `Command` 加上 `Arguments`。

缺失的注册表键是空的可读范围。拒绝访问使该范围不可读。`REG_EXPAND_SZ` 按注册表原文比较，不展开。环境展开属于计划 020。`.lnk` 贡献 MS-SHLLINK LinkInfo 的本地路径。解析失败时保留快捷方式路径，并标记 `link-unresolved`。消失的 RunOnce 值是 `startup.removed`。采集器不声称该命令已经执行。脚本正文的变化不是启动项登记变化。那项观测属于计划 021。

Linux 的覆盖范围是主机文件。它不是 Windows 启动项。采集器的环境被清空，因此 `XDG_CONFIG_DIRS` 未设置，系统自启动目录是 `/etc/xdg/autostart`。用户目录是 `getpwuid_r` 得到的 passwd 家目录加上 `/.config/autostart`。不使用 `HOME` 和 `XDG_CONFIG_HOME`。同名用户 desktop 文件覆盖系统文件。`Hidden=true` 或 `X-GNOME-Autostart-enabled=false` 表示禁用。不评估 `OnlyShowIn`。Cron 读取 `/etc/crontab`、`/etc/cron.d`、`/etc/cron.hourly`、`/etc/cron.daily`、`/etc/cron.weekly`、`/etc/cron.monthly`，以及存在时的用户池 `/var/spool/cron/crontabs` 和 `/var/spool/cron`。池目录存在但无法读取时，cron 范围标记为不可读。systemd timer 读取 `/usr/lib/systemd/system`、`/lib/systemd/system`、`/run/systemd/system`、`/etc/systemd/system`、`/etc/systemd/user` 和 `$config/systemd/user`。只有 `timers.target.wants` 中存在同名标记时，timer 才算启用。`ExecStart` 从配对的 `.service` 读取。采集器不调用 `systemctl` 或 `crontab`。

身份是 Windows 的 `MachineGuid` 加 `BootId`，或 Linux 的 `/etc/machine-id` 加 `/proc/sys/kernel/random/boot_id`。空设备不导出。`identity:5` 保留已经知道的纪元。boot id 变化不清空已记住的启动事实。不同的机器 id 开始新的基线，并且不为上一台机器发出删除。不跟随目录符号链接。文件符号链接会被读取。文件读取在 1 MiB 处停止。非 UTF-8 文本按有损方式解码，因此该文件不会被当成已删除。每一种最多保留 1024 条事实，按 id 排序并去重。被截断的采样不编造删除。不可读的启动范围发出一条缺口和 `startup.permission`。不可读的任务范围发出 `task.permission`。

## 事件

第一次完整采样对已经存在的事实发出 `startup.state` 或 `task.state`。之后的采样发出 `startup.added`、`startup.removed`、`startup.changed`、`startup.enabled`、`startup.disabled`、`task.added`、`task.removed`、`task.changed`、`task.enabled` 和 `task.disabled`。变化事件在先前命令放得进证据标识时包含 `before=`。归因是 `Observed`。

## 仍保留的限制

这台 Linux 编写主机没有观察到 Windows 的 Run 键、启动文件夹、`StartupApproved`、快捷方式本地路径和任务计划程序。该缺口是 [OI-044](../../records/OPEN_ITEMS.md#oi-044)。通过 Linux 自启动和 timer 测试不能关闭它。Windows Server 2022 不是 Windows 10 或 Windows 11（[OI-001](../../records/OPEN_ITEMS.md#oi-001)）。E2E-03 和 E2E-14 未完成。服务控制管理器观测仍属于 [OI-043](../../records/OPEN_ITEMS.md#oi-043)。[ERR-021](../../records/modules/strata-services.md#err-021) 仍留在服务登记中。本计划不复测它，也不关闭它。
