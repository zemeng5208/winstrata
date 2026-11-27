# StrataScript 错误登记

[English](../../../records/modules/strata-script.md) · [全局登记](../ERRORS.md) · [脚本插件](../../architecture/SCRIPT.md)

`plugins/strata-script` 的权威错误记录。

[ERR-020](#err-020) 已解决，并留在这份历史里。[隔离](../../architecture/SCRIPT.md#containment)中的 Windows AppContainer 缺口是 [OI-041](../OPEN_ITEMS.md#oi-041)，不是本登记中的错误。Windows 脚本页是 [OI-040](../OPEN_ITEMS.md#oi-040)。

<a id="err-020"></a>

## ERR-020

- 错误编号: ERR-020
- 发现时间: 2026-11-27T21:13:42Z
- 关联 PR: GitHub PR #34，计划 027，头 `e08cf7ee2dfe42303276a0426d7cec46a6f02867`
- 环境: GitHub Actions `windows-2022`，Windows Server 2022
- 复现方法: `cargo test --locked --workspace` 运行到 `editor_languages_debug_triggers_logs_permissions_and_empty_presets`
- 证据: 测试在 `plugins/strata-script/tests/workbench.rs:237` 恐慌，信息是 `isolation shim is not beside the script program`。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46068679662/job/140740093587)。该头的 Linux 工作区检查通过。已发布的 `winstrata/merge-policy` 状态失败，描述是 “Plan 022 must merge before plan 027”。这个顺序结果是预期的，不是本缺陷。
- 根因: 测试要求 `winstrata_shim.dll` 位于脚本程序旁边。`cargo test` 不编译只有 cdylib 的垫片包。Windows 作业不会先做 Linux 那次会把 `libwinstrata_shim.so` 放到程序旁边的工作区构建。Windows 隔离不加载这个文件。
- 影响: Windows 工作区检查在脚本运行前失败。随后 Workspace 检查因 Windows 失败而失败。
- 修复计划: 只在 Unix 上要求垫片文件，因为 Landlock 使用它。保留 Windows 上的工作台断言。不把 [OI-041](../OPEN_ITEMS.md#oi-041) 标成已通过。
- 复测结果: 头 `a66da7b749ef5657d55f72cccd2d699783dd4a6f` 上，Windows Server 2022 的 `cargo test --locked --workspace` 通过。`editor_languages_debug_triggers_logs_permissions_and_empty_presets` 通过。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46069484949/job/140742722189)。这次运行不关闭 [OI-040](../OPEN_ITEMS.md#oi-040) 或 [OI-041](../OPEN_ITEMS.md#oi-041)。
- 当前状态: 已解决
