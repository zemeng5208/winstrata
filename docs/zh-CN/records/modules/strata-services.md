# 服务插件错误登记

[English](../../../records/modules/strata-services.md) · [全局登记](../ERRORS.md)

`plugins/strata-services` 的权威错误记录。

<a id="err-020"></a>

## ERR-020

- Error ID: ERR-020
- 发现时间: 2026-11-27T20:37:52Z
- 关联 PR: GitHub PR #30，计划 022，头 `20b7bb1502fafde469dbec45d02294ecd1d21d0a`
- 环境: GitHub Actions `windows-2022`，Windows Server 2022
- 复现方法: `cargo test --locked --workspace` 中的 `service_control_manager_observes_a_driver_and_denies_a_restricted_token`
- 证据: 受限令牌调用 `capture`，返回 `Os { code: 5, kind: PermissionDenied, message: "Access is denied." }`，而不是 `permission_denied` 的快照。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46064710156/job/140727118812)。同一作业中 `missing_image_probe_records_creation_start_type_and_deletion` 通过。Linux 工作区检查通过。Workspace 检查因 Windows 失败而失败。
- 根因: `capture` 在打开服务控制管理器之前读取 `MachineGuid`。受限令牌被拒绝读取该注册表值，因此管理器没有被打开，权限缺口没有被记录。
- 影响: Windows 工作区检查失败。拒绝访问的枚举没有被记成 `service.permission`。
- 修复计划: 身份读取被拒绝时仍然打开管理器。管理器拒绝访问仍是 `permission_denied`，清单为空。如果管理器能打开，服务行和驱动行保留，快照同时记录 `identity:5`。这个缺口不替换跟踪器已经知道的纪元。
- 重测结果: 在头 `1987f4826f92aaf5794d4678d3d4b43872932a97` 上，Windows Server 2022 通过了 `service_control_manager_observes_a_driver_and_denies_a_restricted_token`。受限采集设置了 `permission_denied` 并返回空清单。随后一次不受限采集仍然看到驱动。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46068007014/job/140737899687)。同一作业中 `missing_image_probe_records_creation_start_type_and_deletion` 失败。那次失败是 [ERR-021](#err-021)，不重新打开本缺陷。
- 当前状态: 已解决

<a id="err-021"></a>

## ERR-021

- Error ID: ERR-021
- 发现时间: 2026-11-27T21:07:32Z
- 关联 PR: GitHub PR #30，计划 022，头 `1987f4826f92aaf5794d4678d3d4b43872932a97`
- 环境: GitHub Actions `windows-2022`，Windows Server 2022
- 复现方法: `cargo test --locked --workspace` 中的 `missing_image_probe_records_creation_start_type_and_deletion`
- 证据: `plugins/strata-services/tests/scm.rs:161` 的断言要求 `WinStrata022Probe` 有一条 `service.start-type` 事件，实际计数是 0。`ChangeServiceConfigW` 已经返回成功。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46068007014/job/140737899687)。同一测试程序通过了 `service_control_manager_observes_a_driver_and_denies_a_restricted_token`。Linux 工作区检查通过。Workspace 检查因 Windows 失败而失败。[Workspace 作业](https://github.com/zemeng5208/winstrata/actions/runs/46068007014/job/140738578743)。已发布的 `winstrata/merge-policy` 状态成功。[策略状态](https://github.com/zemeng5208/winstrata/actions/runs/46068030319)。
- 根因: 测试在 `ChangeServiceConfigW` 成功后只采集一次，就要求出现 `service.start-type`。跟踪器只在已保存的启动类型发生变化时发出该事件。那次快照的启动类型、配置错误、来源失败和权限标志都没有被记录，因此计数为零的具体原因未知。`QueryServiceConfigW` 使用了 `Vec<u8>`。`QUERY_SERVICE_CONFIGW` 含有指针，这次读取没有对齐，属于未定义行为。读错的启动类型可能与上一份样本相同。来源缺口，或仍然显示旧启动类型的样本，也会得到零条事件。
- 影响: 该头上的 Windows 工作区检查和 Workspace 检查失败。在这次 Windows 探测再次通过之前，合并保持阻断。[OI-040](../OPEN_ITEMS.md#oi-040) 保持开放。E2E-03 和 E2E-14 保持开放。这不是 [ERR-019](../ERRORS.md#err-019)，也不重新打开 [ERR-020](#err-020)。
- 修复计划: 把配置缓冲区按 `u64` 分配，使结构对齐。探测测试最多等待 2 秒，直到一次采集报告自动启动，且没有配置错误、也没有来源失败，然后要求一条 `service.start-type` 事件。两项服务控制管理器测试共用一把锁，因此不会同时枚举。断言保留。
- 重测结果: 尚未执行。这台 Linux 编写主机不能打开服务控制管理器。
- 当前状态: 未解决
