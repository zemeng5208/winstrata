# StrataStorage 错误登记

[English](../../../records/modules/strata-storage.md) · [全局登记](../ERRORS.md) · [存储插件](../../architecture/STORAGE.md)

`plugins/strata-storage` 的权威错误记录。

[ERR-025](#err-025) 未解决。Windows 测试缺口仍是 [OI-058](../OPEN_ITEMS.md#oi-058)。作业 140803723177 不关闭它。Windows 存储页是 [OI-059](../OPEN_ITEMS.md#oi-059)。托盘警告是 [OI-057](../OPEN_ITEMS.md#oi-057)。MCP 配置是 [OI-060](../OPEN_ITEMS.md#oi-060)。[OI-024](../OPEN_ITEMS.md#oi-024) 仍是待所有者决定的事项。[OI-051](../OPEN_ITEMS.md#oi-051) 仍属于计划 026。本文件不改写它们。[ERR-019](../ERRORS.md#err-019) 至 [ERR-024](strata-script.md#err-024) 保持已有记录。

<a id="err-025"></a>

## ERR-025

- 错误编号: ERR-025
- 发现时间: 2026-11-28T00:35:38Z
- 相关 PR: GitHub PR #39，计划 030，头 `f4b27ac0f128ecf5548f9b32f0aab888112bff06`
- 环境: GitHub Actions `windows-2022`，Windows Server 2022。[Windows 作业 140803723177](https://github.com/zemeng5208/winstrata/actions/runs/46087953366/job/140803723177)，运行 46087953366 的第 2 次尝试
- 复现方法: `cargo test --locked --workspace` 运行到 `thresholds_survive_restart_and_confirmed_deletion_updates_indexes`
- 证据: 测试在重新打开 Core 之后，于 `plugins/strata-storage/tests/cleanup.rs:253` 的 `core.configure_storage(...)` 崩溃。`decline_storage("capture")` 已经成功。错误是 `PermissionDenied`，`storage must be owned by the current user with its private DACL`。该头上的 Linux 工作区检查通过。第 1 次尝试，[作业 140801875313](https://github.com/zemeng5208/winstrata/actions/runs/46087953366/job/140801875313)，在 `plugins/strata-process` 超时，没有运行这项测试。那次超时不是这个缺陷。本计划不修改那个包。Workspace 检查因 Windows 失败而失败。已发布的 `winstrata/merge-policy` 状态失败，描述是 “PR is in draft”。这个草稿结果是预期的，不是这个缺陷。
- 根因: 插件用 `OpenOptions` 创建 `policy.json`，没有安全描述符。`Core::configure_storage` 是通过 `open_existing` 更新已有 `policy.json` 的调用。该项检查要求所有者是令牌用户，并且 DACL 是私有形式。`storage_notice` 和 `decline_storage` 不打开这个文件，所以重启后它们成功。在提升后的 Windows 令牌上，没有显式所有者的新建文件归 Administrators 所有，DACL 来自继承。这台主机没有读取运行器上的安全描述符。这次拒绝和创建路径与该项检查相符。检查本身没有放宽。
- 影响: Windows 工作区检查在存储集成测试内失败。Workspace 检查随之失败。[OI-058](../OPEN_ITEMS.md#oi-058) 保持开放，因为这次作业没有通过该测试。
- 修复计划: 用 Core 私有文件所用的显式所有者和受保护 DACL `O:<user>D:P(A;;FA;;;<user>)` 创建插件文件。不放宽 `open_existing`。不修改计划 018。
- 复测结果: 尚未在 Windows 上运行。修复之后，Linux 编写主机通过了 `thresholds_survive_restart_and_confirmed_deletion_updates_indexes`。这次通过不关闭本记录。`RUSTFLAGS="-Dwarnings" cargo check --target x86_64-pc-windows-msvc --offline -p winstrata-strata-storage` 编译了插件。这不是 Windows 测试。
- 当前状态: 未解决
