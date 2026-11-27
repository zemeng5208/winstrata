# 服务错误登记

[English](../../../records/modules/services.md) · [全局登记](../ERRORS.md)

`services` 的权威错误记录。crate 目录是 `services/`。

<a id="err-017"></a>

## ERR-017

- Error ID: ERR-017
- 发现时间: 2026-11-27。第一次 Windows 失败在 `4f7dbf1254b647e92d1b06110cc3f4dfe86c8a8d`。第二次 Windows 失败在 `c056e32389f0e2a75ec0e1407d0e253a5cdde8ab`。两次都在计划 008 期间。
- 关联 PR: GitHub PR #12，计划 008，审查头 `fe4b59117f7b3948f2e1a6ecdcc181ab11ea9795` 之前
- 环境: GitHub Actions `windows-2022`
- 复现方法: `cargo test --locked --workspace` 中的四项服务管道测试
- 证据: `4f7dbf1` 上，三个客户端因为 `listen` 返回时管道尚不存在而得到 Win32 错误 2。第四项测试拒绝了受限令牌，随后的普通客户端得到 Win32 错误 232。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/45998537771/job/140503975068)。`c056e32` 上，三个客户端看到流结束，或看到没有原生代码的传输错误，普通客户端再次得到 Win32 错误 232。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/45999133804/job/140506021647)。两个提交的 Linux 都通过。
- 根因: 监听器在管道实例存在之前返回。纠正之后，客户端没有请求模拟权限，服务因此关闭管道而不是应答。
- 影响: 两个头的 Windows 工作区检查都失败
- 修复计划: 在 `listen` 返回前创建实例，并在轮询期间保持它打开。客户端以 `SECURITY_IMPERSONATION` 打开管道。计划 008 合并前已实现。
- 重测结果: `fe4b59117f7b3948f2e1a6ecdcc181ab11ea9795` 通过 7 项 Windows 服务测试，包括管道交换和受限令牌拒绝。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/45999706617/job/140507919585)。
- 当前状态: 已解决

没有登记其他服务错误。`winstrata-service` 没有注册到服务控制管理器，这是 [OI-014](../OPEN_ITEMS.md#oi-014)，不是一次已运行测试的已确认功能失败。
