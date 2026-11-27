# StrataScript 错误登记

[English](../../../records/modules/strata-script.md) · [全局登记](../ERRORS.md) · [脚本插件](../../architecture/SCRIPT.md)

`plugins/strata-script` 的权威错误记录。

[ERR-022](#err-022) 已解决，并留在这份历史里。[ERR-024](#err-024) 未解决。[隔离](../../architecture/SCRIPT.md#containment)中的 Windows AppContainer 缺口是 [OI-045](../OPEN_ITEMS.md#oi-045)，不是本登记中的错误。Windows 脚本页是 [OI-044](../OPEN_ITEMS.md#oi-044)。计划 022 拥有 [ERR-020](strata-services.md#err-020)。

<a id="err-022"></a>

## ERR-022

- 错误编号: ERR-022
- 发现时间: 2026-11-27T21:13:42Z
- 关联 PR: GitHub PR #34，计划 027，头 `e08cf7ee2dfe42303276a0426d7cec46a6f02867`
- 环境: GitHub Actions `windows-2022`，Windows Server 2022
- 复现方法: `cargo test --locked --workspace` 运行到 `editor_languages_debug_triggers_logs_permissions_and_empty_presets`
- 证据: 测试在 `plugins/strata-script/tests/workbench.rs:237` 恐慌，信息是 `isolation shim is not beside the script program`。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46068679662/job/140740093587)。该头的 Linux 工作区检查通过。已发布的 `winstrata/merge-policy` 状态失败，描述是 “Plan 022 must merge before plan 027”。这个顺序结果是预期的，不是本缺陷。
- 根因: 测试要求 `winstrata_shim.dll` 位于脚本程序旁边。`cargo test` 不编译只有 cdylib 的垫片包。Windows 作业不会先做 Linux 那次会把 `libwinstrata_shim.so` 放到程序旁边的工作区构建。Windows 隔离不加载这个文件。
- 影响: Windows 工作区检查在脚本运行前失败。随后 Workspace 检查因 Windows 失败而失败。
- 修复计划: 只在 Unix 上要求垫片文件，因为 Landlock 使用它。保留 Windows 上的工作台断言。不把 [OI-045](../OPEN_ITEMS.md#oi-045) 标成已通过。
- 复测结果: 头 `a66da7b749ef5657d55f72cccd2d699783dd4a6f` 上，Windows Server 2022 的 `cargo test --locked --workspace` 通过。`editor_languages_debug_triggers_logs_permissions_and_empty_presets` 通过。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46069484949/job/140742722189)。这次运行不关闭 [OI-044](../OPEN_ITEMS.md#oi-044) 或 [OI-045](../OPEN_ITEMS.md#oi-045)。
- 当前状态: 已解决

<a id="err-024"></a>

## ERR-024

- 错误编号: ERR-024
- 发现时间: 2026-11-27T23:34:37Z
- 关联 PR: GitHub PR #38，计划 029，头 `e3dcdd2a89df501b885f3941f4d5312c035126d8`
- 环境: GitHub Actions `windows-2022`，Windows Server 2022。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46083084398/job/140786337911)
- 复现方法: `cargo test --locked --workspace` 运行到 `editor_languages_debug_triggers_logs_permissions_and_empty_presets`
- 证据: 测试在 `plugins/strata-script/tests/workbench.rs:379` 恐慌。`debug-start` 返回 `{"code":"faulted","ok":false,"presets":0}`。该作业随后终止了一个遗留的 `python` 进程。该头的 Linux 工作区检查通过。[Linux 作业](https://github.com/zemeng5208/winstrata/actions/runs/46083084398/job/140786338253)。Workspace 检查因 Windows 失败而失败。[Workspace 作业](https://github.com/zemeng5208/winstrata/actions/runs/46083084398/job/140786937292)。已发布的 `winstrata/merge-policy` 状态失败，描述是 “PR is in draft”。这个草稿结果是预期的，不是本缺陷。
- 根因: `wait_pause` 在 `debug/pause.json` 一出现时就把它当成完整内容。调试器先创建这个文件，再写完行号。读到半截内容就变成 `faulted`，子进程被丢弃时没有被终止。
- 影响: Windows 上的脚本调试返回 `faulted`，而不是 `paused`。Windows 工作区检查失败。Workspace 检查随之失败。在 Windows 复测通过之前，这阻塞计划 029。
- 修复计划: 继续读取，直到文件里有行号，或调试等待结束。等待以 `faulted` 结束时终止子进程。Linux 单元测试 `a_partial_pause_file_is_read_after_it_is_complete` 覆盖了从空文件开始的情况。
- 复测结果: Windows 上未知。这台编写主机上的 Linux 单元测试通过，`editor_languages_debug_triggers_logs_permissions_and_empty_presets` 也在 Linux 上再次通过。这些 Linux 运行不关闭本记录。[OI-044](../OPEN_ITEMS.md#oi-044) 和 [OI-045](../OPEN_ITEMS.md#oi-045) 保持开放。
- 当前状态: 未解决
