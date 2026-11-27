# 服务插件错误登记

[English](../../../records/modules/strata-services.md) · [全局登记](../ERRORS.md)

`plugins/strata-services` 的权威错误记录。

<a id="err-019"></a>

## ERR-019

- Error ID: ERR-019
- 发现时间: 2026-11-27T20:37:52Z
- 关联 PR: GitHub PR #30，计划 022，头 `20b7bb1502fafde469dbec45d02294ecd1d21d0a`
- 环境: GitHub Actions `windows-2022`，Windows Server 2022
- 复现方法: `cargo test --locked --workspace` 中的 `service_control_manager_observes_a_driver_and_denies_a_restricted_token`
- 证据: 受限令牌调用 `capture`，返回 `Os { code: 5, kind: PermissionDenied, message: "Access is denied." }`，而不是 `permission_denied` 的快照。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46064710156/job/140727118812)。同一作业中 `missing_image_probe_records_creation_start_type_and_deletion` 通过。Linux 工作区检查通过。Workspace 检查因 Windows 失败而失败。
- 根因: `capture` 在打开服务控制管理器之前读取 `MachineGuid`。受限令牌被拒绝读取该注册表值，因此管理器没有被打开，权限缺口没有被记录。
- 影响: Windows 工作区检查失败。拒绝访问的枚举没有被记成 `service.permission`。
- 修复计划: 身份读取被拒绝时仍然打开管理器。管理器拒绝访问仍是 `permission_denied`，清单为空。如果身份读取被拒绝但管理器能打开，快照记为 `identity:5`，不在未知启动号下发布行。
- 重测结果: 尚未执行。这台 Linux 主机上的 Windows 目标类型检查通过。Windows 测试还没有再次执行。
- 当前状态: 未解决
