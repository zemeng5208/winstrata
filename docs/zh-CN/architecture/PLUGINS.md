# 原生插件契约与生命周期

[English](../../architecture/PLUGINS.md) · [文档索引](../INDEX.md) · [Rust 宿主](../../../core/src/plugins/mod.rs) · [运行时](../../../runtimes/src/main.rs) · [语言运行时](RUNTIMES.md) · [计划 014](../roadmap/PLAN.md#pr-014)

计划 014 增加原生包契约、依赖检查和进程生命周期。25 个官方身份保留在目录中。它们的采集实现、界面、用户选择启用、语言 SDK、操作系统隔离、市场和签名回退仍属于后续计划。

## 契约

版本化记录放在 `protocols`。清单是协议版本 1.0 的命名 JSON 对象。它包含插件 id、显示名、`plugin_version`（`major`、`minor`、`patch`）、`kind`、`runtime`、能力、权限、依赖、事件和 `rescue`。

插件 id 为 1 到 64 字节。它以小写字母开头，随后只使用小写字母、数字、`.` 或 `-`。它不能包含 `..`，也不能以 `.` 结尾。能力、事件和依赖 id 使用同一形式。`exact` 要求版本相等。`compatible` 在主版本 0 时保持同一次版本且修订不低于要求，在主版本 1 及以上时保持同一主版本且次版本和修订不低于要求。更新必须严格更新。

权限为 `event.publish`、`event.subscribe`、`history.read`、`plugin.call` 和 `storage.private`。只有 `strata.rescue` 可以把 `rescue` 设为真。其他清单这样设置时返回 `rescue_isolated`。本宿主安装的包，`kind` 必须是 `third_party`。`runtime` 是 `native`，或[语言运行时](RUNTIMES.md)中的一种。

宿主和插件进程在标准输入输出上一次交换一帧：小端 `u32` 长度加 JSON，长度为 1 到 65,536 字节。操作是 `ping`、`pause`、`resume`、`stop`、`disable`、`emit`、`invoke`、`read_state`、`fault` 和 `result`。文本字段最多 1,024 字节，不能包含控制字符。运行时报告自己的进程 id。宿主把它与 `Child::id()` 比较。

## 宿主

`Core::open()` 仍使用操作系统定位目录。`Core::open_at` 对显式指定定位目录的调用方公开。`Core::open_plugins` 要求已经选择用户目录，并打开 `<用户>/plugins`。控制文件是 `core.json` 旁边的普通私有文件，不在 `vault.wsv` 内。计划 006 的迁移复制用户目录中的普通文件。本计划没有增加插件迁移测试，也不加密这些文件。

| 路径 | 内容 |
|---|---|
| `host.json` | 已安装条目、授权、待授权权限、版本锁定、启用状态，以及 `strata.registry`、`strata.env` 和 `strata.trace` 的启用标志、可用性和范围 |
| `history/<id>.jsonl` | 生命周期动作，保留最近 1,024 行 |
| `packages/<id>/` | 当前运行的包 |
| `recovery/<id>/` | 最近一次安装或成功更新的包 |
| `state/<id>/` | 该插件的私有目录 |
| `staging/<id>/` | 更新期间的候选包 |

包是一层普通文件目录。文件名长 1 到 48 字节，以小写字母开头，只使用小写字母、数字、`.`、`_` 或 `-`。宿主最多接受 16 个文件，每个 64 KiB，并拒绝符号链接。最多安装 256 个插件，最多同时运行 64 个。

原生程序是 `winstrata-runtime`。宿主用绝对路径传入 `--manifest` 和 `--state`，并把工作目录设为私有状态目录。标准错误被丢弃。Unix 上子进程环境被清空。Windows 上子进程保留进程环境，因为计划 004 发现清空环境可能使进程无法创建。每个子进程有一个名为 `ws-plugin` 的线程转发帧。它不占用 Core 调度工作者。关闭宿主时终止仍在运行的子进程，并且不把它们标成故障，因此下次打开时已启用的插件显示为 `stopped`。

## 官方目录

`list` 始终返回这 25 个身份。`strata.registry` 是[注册表插件](REGISTRY.md)里的进程内监视。`strata.env` 是[环境插件](ENV.md)里的进程内监视。`strata.trace` 是[跟踪插件](TRACE.md)里的进程内文件监视。三者都没有包，也都不启动 `winstrata-runtime`。对它们调用 `load`、`update`、`pin`、`grant`、`call` 和 `ping` 返回 `builtin_closed`。对它们调用 `uninstall` 返回 `builtin_retained`。只有 `winstrata-strata-process` 位于运行时旁边时，宿主才登记 `strata.process`，并且在启用前保持禁用。只有 `winstrata-strata-health` 位于运行时旁边时，宿主才登记 `strata.health`，并且在启用前保持禁用。仍没有实现的官方 id，`enable` 返回 `implementation_absent`。每个官方 id 的 `uninstall` 都返回 `builtin_retained`，包括 `strata.registry`、`strata.env`、`strata.trace`、`strata.process` 和 `strata.health`。第三方包不能占用这些 id，也不能把 `kind` 设为 `builtin`。只有 `strata.rescue` 关闭批量启用。

| Id | 名称 | 计划 |
|---|---|---|
| `strata.pulse` | StrataPulse | 24 |
| `strata.net` | StrataNet | 31 |
| `strata.trace` | StrataTrace | 21 |
| `strata.apps` | StrataApps | 35 |
| `strata.registry` | StrataRegistry | 19 |
| `strata.env` | StrataEnv | 20 |
| `strata.process` | StrataProcess | 18 |
| `strata.services` | StrataServices | 22 |
| `strata.startup` | StrataStartup | 23 |
| `strata.security` | StrataSecurity | 37 |
| `strata.snapshot` | StrataSnapshot | 38 |
| `strata.rules` | StrataRules | 39 |
| `strata.health` | StrataHealth | 25 |
| `strata.timeline` | StrataTimeline | 41 |
| `strata.budget` | StrataBudget | 26 |
| `strata.mcp` | StrataMCP | 45 |
| `strata.rescue` | StrataRescue | 47 |
| `strata.filter` | StrataFilter | 28 |
| `strata.disk` | StrataDisk | 32 |
| `strata.script` | StrataScript | 27 |
| `strata.dependency` | StrataDependency | 34 |
| `strata.trust` | StrataTrust | 33 |
| `strata.update` | StrataUpdate | 36 |
| `strata.tag` | StrataTag | 29 |
| `strata.storage` | StrataStorage | 30 |

## 生命周期

`install` 把包复制到 `packages` 和 `recovery`，创建私有状态目录，并让插件保持禁用。声明的权限被授予。`load` 检查每个依赖都已安装且版本兼容。禁用的依赖仍然可以完成加载。`enable` 先把插件记为启用，然后启动它。启动失败时，它可能保持启用，可用性为 `unavailable` 或 `faulted`。

`start` 拒绝禁用的插件、依赖循环，以及已经有另一个运行中插件提供同一能力的情况。它会启动已启用且已停止的依赖。禁用、故障或不可用的依赖返回 `dependency_unhealthy`。子进程必须用自己的进程 id 和清单版本回答 `ping`。退出码 3 是 `unavailable`，不会替换已经在运行的插件。可选的 `startup.json` 在 `ready` 为假时，或 `startup.json` 无效时，运行时在读取标准输入之前以 3 退出。

`pause` 和 `resume` 保留进程，并移除或恢复它的总线订阅。`stop` 要求以 0 退出，并保持插件启用。`disable` 要求以 0 退出，清除启用状态，并保留历史。`fault` 要求以 7 退出。`uninstall` 只适用于第三方插件。历史文件保留。

`update` 先暂存新包。插件正在运行时，第二个进程先对暂存副本做健康检查，然后旧进程才停止。拒绝启动或握手失败会删除暂存并让旧进程继续运行。旧进程干净退出后，宿主写入 `packages` 和 `recovery` 并接管新进程。新增权限留在 `pending`，不会自动授予。`pin` 必须等于已安装版本。锁定会挡住其他更新，直到 `unpin`。

`recover` 在清单可读且插件未故障时返回当前包。否则它恢复 `recovery/<id>`，并在插件已启用时启动它。恢复副本缺失或无效时返回 `dependency_unrecoverable`。`enable_batch` 只启动已经启用、尚未运行且允许批量启动的插件。它不会启用一个被禁用的插件。`strata.rescue` 保持缺席。

## 权限、调用和事件

`grant` 只接受当前处于待授权状态的权限。`revoke` 移除一项授权。如果已安装清单仍然声明该权限，它回到待授权，之后可以再次授予。撤销 `event.subscribe` 会立刻移除该插件的总线订阅。

调用方声明了该能力时，`call` 留在调用方进程。否则调用方需要 `plugin.call`，宿主把调用转给唯一正在运行的提供者。对 `strata.health` 的 `health.observe` 调用会用[自身诊断](HEALTH.md)中的宿主快照替换调用方载荷。其他能力仍使用调用方载荷。`read_private` 需要 `storage.private`，并要求插件读取其状态目录中的一个文件名。运行时拒绝 `..`、符号链接和该目录之外的名称。`emit` 需要 `event.publish` 和已声明的事件类型。发布的事件使用设备 `local-plugin-host`、插件 id 作为提供者、每次打开宿主时的新 epoch，以及从 1 开始的每个插件序列。`poll` 返回已经排进该插件所声明类型的事件。一个插件暂停时，共享总线保持运行。

## 本计划未完成的部分

计划 014 对原生运行时实际使用的隔离是独立进程、私有目录，以及一个插件失败时另一个插件继续运行。这不是 AppContainer、Job Object 或 seccomp 沙箱。语言运行时及其操作系统隔离属于[计划 015](RUNTIMES.md)。官方默认界面属于计划 016。计划 017 把首次启用选择写入 `onboarding.json`，不调用 `enable`，也不启动官方采集器。救援是单独的标志，并且不进入该批量选择。计划 018 提供 `strata.process` 采集实现，见[进程来源](PROCESS.md#plugin)。只有 `winstrata-strata-process` 位于运行时旁边时，宿主才登记它，并且在启用前保持禁用。计划 019 在进程内运行 `strata.registry`。计划 020 在进程内运行 `strata.env`。计划 021 在进程内运行 `strata.trace`。计划 025 只在 `winstrata-strata-health` 位于运行时旁边时运行 `strata.health`，并且在启用前保持禁用。其余采集实现仍属于计划 022 至 024 以及 026 至 047。市场属于计划 043。签名更新和事务回退属于计划 044。救援行为属于计划 047。完整 GUI 替换是 [AG-64](../AGENTS.md#ag-64)，记为计划 053。这里没有实现它，它也不是计划 016 的验收。计划 053 不另加实现计划编号。E2E-06、E2E-11 和 E2E-15 未完成。证据见[工程记录](../records/STATUS.md)。
