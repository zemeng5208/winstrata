# 内核错误登记

[English](../../../records/modules/kernel.md) · [全局登记](../ERRORS.md)

`kernel` 的权威错误记录。

<a id="err-003"></a>

## ERR-003

- Error ID: ERR-003
- 发现时间: 2026-11-27，计划 003 期间。没有记录时钟时间。
- 关联 PR: GitHub PR #4，计划 003
- 环境: Linux x86_64，用固定的 WDK 导入库链接内核 PE32+ 探针
- 复现方法: `cargo dev cross-probe` 链接内核映像
- 证据: 第一次链接失败，因为所选 x64 WDK 导入归档没有导出 `KeQueryInterruptTime`。检查归档后，探针改用文档所列的真实导出 `KeQueryTimeIncrement`。两项探针和 PE 检查重新运行。没有执行 Windows 驱动。
- 根因: 所选导入不是该归档的导出
- 影响: 第一次 Linux 驱动链接失败
- 修复计划: 计划 003 合并前已实现
- 重测结果: 记录中的计划 003 探针完成链接，两次产物逐字节一致
- 当前状态: 已解决

<a id="err-007"></a>

## ERR-007

- Error ID: ERR-007
- 发现时间: 2026-11-27，计划 011 期间 `fcb7e3d61881a87e05d92f76846df86d6fdd7778` 的 Windows 工作区作业
- 关联 PR: GitHub PR #14，计划 011，后来的绿色头之前
- 环境: GitHub Actions `windows-2022`
- 复现方法: `cargo test --locked --workspace`。Core 进入现场交换后，`Observer::collect` 返回 Winsock 错误 10022（`WSAEINVAL`）。
- 证据: [Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46007292420/job/140533560835)。未绑定的 `SOCK_RAW` 在 `recv` 时返回该错误，于是连接表在测试读取之前被丢弃。同一提交的 Linux 通过。
- 根因: 原始套接字没有绑定，`recv` 返回 `WSAEINVAL`，采集器丢弃了连接表
- 影响: Windows 工作区检查失败，因此 Workspace 检查失败
- 修复计划: 把原始套接字绑定到一个非环回 IPv4 地址，并把采样错误留在缺口列表中。计划 011 合并前已实现。
- 重测结果: `ab01883653187bbebd5e9197a80a92b784d89e36` 在 Windows Server 2022 上通过两项现场网络测试
- 当前状态: 已解决

<a id="err-008"></a>

## ERR-008

- Error ID: ERR-008
- 发现时间: 2026-11-27。第一次记录的 Windows 失败在计划 011 的 `34385e9ad0c097d236dc2fce3a2bb675bd4042cb`。同一断言在合并 main `9f70e62` 之后的计划 012 头 `3a4136ece12b511b225812cad51b42d64ffd4649` 上再次失败。
- 关联 PR: 计划 011 和 012，各自审查头之前
- 环境: GitHub Actions `windows-2022`
- 复现方法: `service_control_manager_refuses_to_start_a_missing_kernel_image_and_the_entry_is_removed` 创建 `WinStrata008MissingImage`，`StartService` 失败，`DeleteService` 成功，下一次 `OpenServiceW` 仍能打开该探测服务
- 证据: 计划 011 恐慌信息为 “probe service still exists”。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46015372432/job/140561018902)。计划 012 在 `kernel/src/scm.rs:234` 以同一信息失败。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46004612235/job/140524531045)。`kernel/src/scm.rs` 与该测试曾经通过的 main 提交相同。
- 根因: `DeleteService` 已返回成功且句柄已关闭，但 `OpenServiceW` 立刻仍能找到探测服务。超出这次已记录竞态的服务控制管理器延迟未知。
- 影响: 两个头的 Windows 工作区检查都失败
- 修复计划: 最多等待两秒直到出现 `ERROR_SERVICE_DOES_NOT_EXIST`，关闭检查打开的句柄，探测服务仍在时测试仍然失败。测试没有跳过。计划 011 和 012 合并前已实现。
- 重测结果: `e0abe645789b1f32173612d22ce4459d42191dee` 通过该测试。`804576c7e64fe13685982a59f59e1ad8cb8257cf` 以 9 项内核测试、0 项失败通过该测试。
- 当前状态: 已解决
