# 进程、线程与模块来源

[English](../../architecture/PROCESS.md) · [采样](../../../kernel/src/system.rs) · [跟踪](../../../core/src/process.rs) · [计划 010](../roadmap/PLAN.md#pr-010)

计划 010 在 `winstrata-kernel` 中增加用户态进程采样，并在 `winstrata-core` 中增加进程跟踪。直接前置是计划 008。`CatalogHost` 对 `strata-proc` 仍返回 `image_absent`，不加载驱动。服务 IPC 没有改变。Core 在进程内调用采样器，因为读取 procfs 或 Windows 进程接口不需要新增高权限操作。

## 采样

`capture` 读取调用方令牌可见的一次采样。它不创建、不启动、不停止服务，也不打开内核进程回调。

Linux 的设备标识是 `/etc/machine-id`，启动标识是 `/proc/sys/kernel/random/boot_id`。进程启动时间由 `/proc/stat` 的 `btime` 加上 `/proc/<pid>/stat` 第 22 字段，并用 `sysconf(_SC_CLK_TCK)` 换算。线程是 `/proc/<pid>/task` 中的条目。模块是 `/proc/<pid>/maps` 里按路径归并的可执行文件映射。cgroup 路径中除 `user@*.service` 以外的 `.service` 单元记为服务行。

Windows 的设备标识是 `MachineGuid`，启动标识是注册表 `BootId`，前缀为 `boot:`。进程和线程来自 Toolhelp。启动时间来自 `GetProcessTimes`，映像路径来自 `QueryFullProcessImageNameW`。模块来自按进程建立的模块快照。服务行来自 `EnumServicesStatusExW` 的 `SERVICE_WIN32` 查询，使用其中的进程号。进程号为零不是服务关联。

一次采样最多保留 4,096 个进程、每个进程 1,024 个线程和 64 个模块，以及 1,024 条服务行。超出部分设置对应的截断标志。映像、启动时间、线程或模块无法读取时设置 `limited`，并留空缺失字段。采样器不编造启动时间。服务枚举失败时设置 `services_unavailable`，服务列表为空。

## 跟踪

`ProcessTracker::observe` 在来源 `winstrata.process` 上分配协议事件。来源实例是启动标识。进程实例由启动标识、PID 和启动时间组成。1.0 观察记录没有属性表，因此映像、父进程和服务名保留在 `ProcessNode` 上。不读取命令行。

计划 017 增加 `observe_scoped`。映像不在所选根目录内、名称未被选中或缺少映像的进程会被丢掉。所属进程被丢掉的服务行也会被丢掉。文件扩展名不过滤进程映像。关闭进程监视且没有选中名称时，它不返回事件，也不调用 `observe`。`observe` 本身不变。

实例第一次出现时发出 `process.started`，归因为已观察，证据是 `procfs` 或 `toolhelp`。后续采样仍包含该实例时不再发出启动。实例消失时发出一条 `process.exited`，完整性为部分，原因是 `not_in_snapshot`。再一次采样不重复这条退出。事件时间是采样时间，不是内核退出时间戳。

同一 PID 的启动时间与正在跟踪的实例不同时，关闭旧实例并打开新实例。被截断的采样不把缺失 PID 当成退出，也不替换已跟踪的启动时间。线程和模块使用同一套首次出现与缺席规则。线程或模块列表被截断时不发出移除事件。进程退出不再逐条发出线程和模块退出。

服务行的 PID 已有启动时间时，发出一条 `process.service`。归因是推断，证据是 `service:<name>`。它不是第二条 `process.started`。同一服务名在该实例上不重复发出。没有启动时间的服务 PID 只发出一条部分 `process.service`，不创建 `ProcessRef`。持续可见但没有启动时间的 PID 发出一条 `process.unresolved`，直到它消失。已经跟踪的实例如果仍被列出但没有启动时间，不发出 `process.exited`。退出要等到下一次完整采样不再包含该实例，或给出另一个启动时间。

父进程必须出现在同一次采样中，且其启动时间不晚于子进程，才保留父子链接。发现环时去掉该链接。启动标识或设备标识变化时，先关闭原来的来源，再使用新的来源实例。

## 覆盖范围

这是采样，不是连续跟踪。只在两次采样之间出现并退出的进程没有事件，跟踪器也不为它编造序号缺口。没有实现 ETW 或内核进程回调。服务停止和重启记录仍属于计划 022。文件、注册表和网络采集仍属于计划 009 和 011。运行这个采样器的插件是[计划 018](#plugin)。

## 插件

<a id="plugin"></a>

计划 018 在 `plugins/strata-process` 中运行该采样器。程序名是 `winstrata-strata-process`。它使用现有的原生帧协议：绝对路径 `--manifest` 和 `--state`，一次一帧 JSON。它不新增帧操作，也不新增依赖。

只有当该程序是宿主收到的运行时同目录下的普通文件时，`PluginHost::open` 才登记 `strata.process`。这条记录是内置插件，版本 1.0.0，默认禁用。授予的权限是 `event.publish` 和 `storage.private`。第三方 `install` 仍返回 `builtin_closed`。`uninstall` 仍返回 `builtin_retained`。其余 24 个官方身份在各自的计划之前仍是 `implementation_absent`。

启用后启动采集进程。进程大约每 200 毫秒通过 `capture` 和 `ProcessTracker` 采样一次。成功的采样把跟踪器的协议事件追加到私有状态目录的 `events.jsonl`，并替换 `tree.json`。树里包含父子链接、映像、服务名、线程和模块。启动时间和模块基址写成十进制字符串，因为它们不能精确放进 JSON 数字。日志达到 16 MiB 时，下一次追加前改名为 `events.prev.jsonl`，只保留上一份。跟踪状态留在进程内，因此轮转不会再次发出启动。

`process.started`、线程事件和模块事件保持已观察归因。证据是 `procfs` 或 `toolhelp`。`process.service` 保持推断归因。来源提供者仍是 `winstrata.process`。这些观察不通过 `emit` 发布。帧里的文本字段最多 1,024 字节，而 `emit` 记录的是采集进程，不是被采样的进程。对 `process.observe` 的 `invoke` 返回采样次数和事件条数。

暂停停止新的采样，进程继续存在。恢复后沿用同一个跟踪器。禁用和停止要求采样线程退出，先发送现有的应答帧，再等待名为 `ws-process` 的线程结束，把字节 `joined` 写入 `released`，然后以 0 退出。采样线程恐慌时不写这个标记，进程以 1 退出。宿主仍然只删除语言运行时的 `env` 目录。事件日志和进程树保留。

官方外壳已经列出这个插件。本计划不新增页面、主题或视觉体系。首次启动时的启用选择仍属于计划 017。

没有实现命令行、ETW、内核进程回调，也没有实现只在两次采样之间出现并退出的进程。服务停止记录仍属于计划 022。没有加载已签名的 `strata-proc` 映像。E2E-02 和 E2E-03 未完成。

## 验证

在 Linux x86_64、Rust 1.100.0 上，`RUSTFLAGS="-Dwarnings" cargo test --workspace` 通过 32 项 Core 测试、9 项内核测试、11 项协议库测试、2 项协议集成测试和 7 项服务测试。新增测试读取真实进程表。一个真实子进程被连接到本次测试进程，只出现一次；第二次采样没有再次发出启动；回收后只发出一次退出。进程表中存在至少三个进程的链。这些事件发布到 Core 总线后序号连续，且没有记录丢失。本机这次采样有 33 个进程，没有 `.service` cgroup 单元，因此服务测试确认没有编造间接事件。

复用测试把两个真实启动时间先后放到同一个 PID 上，关闭前者并打开后者。这个组合是跟踪器的输入安排。测试期间操作系统没有回收 PID。`cargo check --target x86_64-pc-windows-msvc --tests` 在拒绝警告的条件下对 Toolhelp、注册表和服务控制管理器代码做了类型检查。这些 Windows 测试没有在那台主机上执行。后来的 Windows Server 2022 作业运行了 `real_process_tree_pid_lifecycle_and_dedup`。它在计划 010 的证据和计划 013 的送审头上通过。main `a2471d5fc13f0a307ee39d2629c454aea4537809` 上失败。见 [ERR-005](../records/modules/core.md#err-005)。计划 052 已合并为 `004974215e239f96160b47d3eef40a84c316e802`，该提交的 Windows 工作区检查通过。先前失败的根因仍然未知。没有加载已签名的 `strata-proc` 映像。E2E-02 和 E2E-03 仍待完成。计数和 Windows 运行结果记入[工程记录](../records/STATUS.md)。

计划 052 不改变这条退出规则。`real_process_tree_pid_lifecycle_and_dedup` 仍然要求子进程被回收后只有一条 `process.exited`。测试会等到完整采样不再包含该实例，或该实例被另一个启动时间替换。仍列出该 PID 但没有正的启动时间的采样不是这条退出，被截断的采样也不是。主分支失败日志没有包含那次采样，因此记录不声称当时是这两种条件中的哪一种。计划 051 不修改这个测试。

计划 018 在这台 Linux x86_64 主机上用 Rust 1.100.0 从 `plugins/strata-process` 运行该采样器。换基到 `88249119943516437819d93eb7f7bcd50c3b45b8` 之后，`RUSTFLAGS="-Dwarnings" cargo test --locked --workspace` 通过 66 项 Core 测试、23 项内核测试、14 项协议库测试、2 项协议集成测试、3 项语言测试、5 项运行时生命周期测试、7 项服务测试、5 项桌面测试和 1 项进程插件测试。插件测试启动 `/bin/sleep 120`，观察到一次启动、一次线程事件、一次模块事件、`tree.json` 中的父链接、一次退出，以及禁用后 `released` 中的 `joined`。这台主机没有运行 Windows 测试。E2E-02 和 E2E-03 仍待完成。
