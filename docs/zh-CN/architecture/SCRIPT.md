# 脚本插件

[English](../../architecture/SCRIPT.md) · [文档索引](../INDEX.md) · [插件宿主](PLUGINS.md) · [语言运行时](RUNTIMES.md) · [错误登记](../records/modules/strata-script.md) · [计划 027](../roadmap/PLAN.md#pr-027)

计划 027 是 `plugins/strata-script` 中的官方插件 `strata.script`。显示名是 StrataScript。程序是 `winstrata-strata-script`。直接前置是计划 015 和 016。进程使用现有的长度前缀 JSON 帧。它不新增帧操作。

## 登记

只有当 `winstrata-strata-script` 是传给 `PluginHost::open` 的运行时旁边的普通文件时，宿主才登记 `strata.script`。登记的条目是内置插件，版本 1.0.0，在启用前保持禁用。能力是 `script.workbench`。权限是 `event.subscribe`、`event.publish`、`storage.private` 和 `history.read`。事件是 `script.started`、`script.finished` 和 `script.faulted`。第三方 `install` 仍返回 `builtin_closed`。`uninstall` 仍返回 `builtin_retained`。

对 `script.workbench` 的 `invoke` 携带最多 1,024 字节、不含控制字符的紧凑 JSON 载荷。脚本源码以 base64 传递，可以是一整段，也可以按编号分成多段，因为帧的文本字段放不下原始换行。

## 预设

包目录里只有 `manifest.json`。crate、包和 SDK 都不附带预设脚本。启动时插件创建空的 `presets` 目录，并把 `preset.count` 写成单个字符 `0`。该目录一旦含有文件，工作台命令就返回 `preset_present`，并且不运行脚本。

## 工作台

工作台把用户脚本存在插件私有状态中。语言是 `python`、`javascript` 和 `typescript`。新建脚本处于禁用。编辑器返回记号种类、关键字和标识符补全，以及空脚本、未闭合字符串和未配对括号的诊断。每次编辑都增加版本。`hot-reload` 替换已启用脚本的源码，下一次运行使用这份源码。`export` 和 `import` 各移动一份脚本。导入的脚本保持禁用，并且不是预设。

## 执行

运行、调试和触发执行宿主在 `bind_languages` 之后、启动时传入的真实解释器：`--python`、`--python-prefix`、`--node` 和 `--node-prefix`。宿主不在插件进程内搜索 `PATH`，因为 Unix 上插件的环境被清空。缺少这些参数时，执行返回 `toolchain_missing`。已声明的模块记号在用户脚本之前导入。缺少模块时返回 `dependency_missing`，用户脚本不会启动。每次运行限时 1.2 秒，到时终止子进程，结果是 `timeout`。日志是私有状态里的 JSON 行。禁用会停止调度器，终止子进程，并把 `joined` 写入 `released`。

## 权限

工作台使用五个既有权限名：`event.publish`、`event.subscribe`、`history.read`、`plugin.call` 和 `storage.private`。`declare` 把一个名称放入待授权。`grant` 只接受仍在待授权中的权限。`revoke` 把它放回待授权。一次运行要记录 `emit` 之前必须已有 `event.publish`。事件触发执行之前必须已有 `event.subscribe`。子进程获得其私有目录的写访问之前必须已有 `storage.private`。

## 触发

手动触发是 `run`。事件触发是 `ingest`，并且事件记号必须匹配。定时触发的 `every_ms` 至少为 200，只在本进程内的后台线程上运行。条件触发的 `field` 是 `count`，`cmp` 是 `gte` 或 `eq`。插件暂停，以及脚本暂停或禁用，都会跳过触发。`strata.process` 和 `strata.registry` 的采集保持不变。

## 调试

Python 在真实源码上使用 `sys.settrace`。JavaScript 和 TypeScript 在 Node 下运行，行钩子位于私有 `debug` 目录中生成的引导文件里。该引导文件不是预设。TypeScript 使用 Node 的 `--experimental-strip-types`。`debug-start`、`debug-step`、`debug-continue` 和 `debug-stop` 使用一个暂停文件和一个控制文件。

<a id="containment"></a>

## 隔离

当 `libwinstrata_shim.so` 位于运行时旁边时，每个用户脚本子进程使用与[计划 015](RUNTIMES.md)相同的 Landlock 和 `LD_PRELOAD` 垫片。读访问覆盖解释器前缀、加载器路径和插件状态。调试期间的写访问覆盖 `debug`。只有授予 `storage.private` 时，写访问才覆盖 `private`。垫片或 Landlock 无法安装时不启动子进程，结果是 `isolation_unavailable`。本计划不把 Windows 子进程放入 AppContainer。这个缺口是 [OI-045](../records/OPEN_ITEMS.md#oi-045)，不记为成功。

## 官方页面

官方页面组合现有的 shadcn-admin [Button](../../../packages/ui/vendor/shadcn-admin/src/components/ui/button.tsx)、[Input](../../../packages/ui/vendor/shadcn-admin/src/components/ui/input.tsx) 和 [Badge](../../../packages/ui/vendor/shadcn-admin/src/components/ui/badge.tsx)，再加一个 textarea。它不加入 Fluent UI、Astryx 或 Monaco，也不建立新的视觉体系。它不为该插件发送 `enable` 或 `grant`。桥接命令 `script-workbench` 只在插件已经运行时转发载荷。

## 本计划之外

计划 026、028、038、043 和 045 不在本计划内。过滤回退、快照比较、统一市场和 MCP 也不在本计划内。[E2E-07](../roadmap/ACCEPTANCE.md#e2e-07) 和 [E2E-12](../roadmap/ACCEPTANCE.md#e2e-12) 仍待完成。
