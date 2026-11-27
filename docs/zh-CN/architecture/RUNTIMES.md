# Python、TypeScript 与 JavaScript 运行时

[English](../../architecture/RUNTIMES.md) · [文档索引](../INDEX.md) · [插件](PLUGINS.md) · [SDK](../../../sdk) · [计划 015](../roadmap/PLAN.md#pr-015)

计划 015 增加三种语言运行时、对应 SDK、项目依赖检查和操作系统隔离。原生契约和进程生命周期仍在[插件](PLUGINS.md)。编辑器、触发器和预设属于[计划 027](SCRIPT.md)。市场属于计划 043。

## 运行时

第三方清单可以把 `runtime` 设为 `native`、`python`、`javascript` 或 `typescript`。其他记号返回 `unsupported_runtime`。更新不能改变运行时。语言包必须包含 `plugin.py`、`plugin.mjs` 或 `plugin.ts`。尚未绑定工具链就安装语言包时返回 `toolchain_missing`。

`bind_languages` 核对两份项目声明。Node.js 必须等于 `package.json` 的 `engines.node`，当前是 24.22.3。Python 必须不低于 `toolchains/runtimes.json` 的 `python_minimum`，并且保持该主版本。调用方传入解释器的绝对路径和 SDK 目录。宿主不隐式搜索用户安装，也不把解释器复制进仓库。与声明不符的二进制返回 `toolchain_mismatch`。

在 Linux 上，Python 从插件私有状态目录里的 `venv` 启动，创建时使用 `--without-pip`。在 Windows 上，这个 `venv` 可执行文件只是启动器，而作业只允许一个进程，因此 Python 直接使用项目声明的解释器。两种情况下 SDK 都以 `-s -B -E` 运行它，然后只加入该插件的私有模块目录；`modules.json` 列出的模块才加入项目目录。JavaScript 和 TypeScript 使用绑定的 Node.js。TypeScript 使用 Node 自带的类型剥离。私有模块和目录模块按名称从 `state/node` 和 `plugins/catalog/node` 加载。不在其中的模块视为缺失。用户站点包和另一个插件的目录不在搜索路径上。

`modules.json` 可选。其中的 `modules` 名称是小写记号。列出的模块必须已经存在于 `plugins/catalog/python` 或 `plugins/catalog/node`。宿主不会下载它。停止、禁用、卸载和关闭宿主时删除 Python 的 `env` 目录。私有文件和历史保留。下次启动重新创建环境。

SDK 使用与 `winstrata-runtime` 相同的长度前缀帧。它们用自己的进程 id 和插件版本回答 `ping`，并实现 `pause`、`resume`、`stop`、`disable`、`fault`、`emit`、`invoke` 和 `read_state`。`startup.json` 的 `ready` 不是真，或文件无效时，进程在读取标准输入前以状态 3 退出。

## Linux 隔离

语言进程启动前，宿主建立 Landlock 规则集，并用 `LD_PRELOAD` 加载 `libwinstrata_shim.so`。该库在解释器主函数之前运行。它套用规则集，安装 seccomp 过滤，并关闭除标准输入、输出和错误以外的继承描述符。Landlock 或过滤无法安装时，进程退出，宿主返回 `isolation_unavailable`，不会在没有隔离的情况下继续。

规则集允许读取并执行解释器前缀、SDK、包、动态链接目录、`/dev/null`、`/dev/urandom` 和 `/etc/ssl`。它允许读写该插件的状态目录。只有声明了目录模块的插件才能读取对应目录。过滤拒绝套接字、`execve`、`fork`、`vfork`、挂载和命名空间切换。使用 `clone` 的线程仍然可以工作。在允许目录内已经打开的文件会一直可读到进程退出；宿主权限变化时，操作系统不会收回该句柄。宿主权限对下一帧仍然生效：撤销 `event.subscribe` 会立刻移除总线订阅。

原生 `winstrata-runtime` 进程保持原样。它仍是带私有目录的独立进程，不使用这份 Landlock 策略。

## Windows 隔离

Windows 宿主创建没有额外能力的 AppContainer 配置文件，以及最多一个活动进程并在关闭时终止的作业。它把包 SID 的读取和执行授予解释器前缀、SDK、包和目录，并把读写授予状态目录。这条允许项合并进现有 DACL。受保护的私有目录保持受保护，所有者的允许项也保留。宿主仍然拒绝任何不是该插件 AppContainer 配置文件的额外允许项。解释器在该配置中以挂起状态创建，加入作业后再恢复。Windows 上的 Python 直接启动项目解释器，因为 venv 可执行文件只是启动器，而作业只允许一个进程。环境块来自当前用户令牌。只有标准输入和输出管道被继承。撰写本计划的 Linux 主机没有执行这条路径。Windows 测试是 `runtimes/tests/languages_windows.rs`。

## 仍属后续计划的部分

计划 027 提供脚本编辑器、调试、触发器、日志、权限和空预设检查。Windows 脚本页仍是 [OI-044](../records/OPEN_ITEMS.md#oi-044)。这些子进程的 Windows AppContainer 仍是 [OI-045](../records/OPEN_ITEMS.md#oi-045)。统一市场属于计划 043。签名更新属于计划 044。E2E-06、E2E-07 和 E2E-12 未完成。允许目录内已经打开的句柄不会在进程中途被收回。撰写主机没有在 Windows 10 或 11 客户端上运行这套隔离。证据见[工程记录](../records/STATUS.md)。
