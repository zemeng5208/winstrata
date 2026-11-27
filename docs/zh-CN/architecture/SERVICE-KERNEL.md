# 受控服务、受控 IPC 与驱动管理

[English](../../architecture/SERVICE-KERNEL.md) · [服务](../../../services/src/lib.rs) · [驱动管理](../../../kernel/src/lib.rs) · [计划 008](../roadmap/PLAN.md#pr-008)

计划 008 增加 `winstrata-service` 与 `winstrata-kernel`。服务与当前用户交换带版本、带长度前缀的 JSON。kernel crate 跟踪四个驱动订阅。它不采集文件、注册表、进程或网络活动，产品宿主也不加载内核映像。

## 权限

`Service::listen_catalog` 从操作系统读取身份和权限。Linux 身份为 `uid:` 加有效用户号，权限为 `NotWindows`。Windows 身份为进程令牌 SID。仅当 `TokenElevation.TokenIsElevated` 非零时，权限才是 `WindowsElevated`。

订阅和释放要求 `WindowsElevated`。状态查询和审计不要求。Linux 进程和未提升的 Windows 令牌会在调用驱动宿主之前收到 `privilege_unavailable`。`GuiClient` 只提供 `status`。服务端还会以 `permission_denied` 拒绝 `gui`、`plugin` 和 `script` 角色的订阅、释放和审计。

操作系统认证的是用户，不是进程角色。以服务所有者身份运行的进程可以声称角色为 `core`。该限制记为 AS-005。插件能力隔离仍属于 PR 014 和 PR 015。

## 传输

一帧是小端 `u32` 长度，随后是最多 16 KiB 的 UTF-8 JSON。单次读写最多等待 2 秒。接受循环一次只服务一个连接。Windows 在 `listen` 返回前创建管道实例。200 毫秒的等待只检查停止标志，不关闭该实例。下一个实例尚未开始监听时，客户端最多重试 2 秒。管道关闭后的读取，包括 Win32 错误 232，视为流结束。

Linux 使用 Unix 域套接字。临时端点是权限 `0700` 的目录，其中套接字权限为 `0600`；关闭时删除该目录。默认端点在确认运行时目录为私有后使用 `$XDG_RUNTIME_DIR/winstrata/service.sock`。启动时会移除同名的旧套接字。关闭时不删除默认套接字，因此后启动的监听者不会被清掉。`SO_PEERCRED` 提供对端用户，其 uid 必须等于服务的有效 uid。凭据无法读取或 uid 不同时，连接被丢弃且 `faults` 增加。这些字节不会被解析，也不写审计记录。

Windows 使用重叠 I/O 的命名管道。临时名为 `\\.\pipe\winstrata-<pid>-<n>`。默认名为 `\\.\pipe\WinStrata-<sid>`。DACL 为 `D:P(A;;GA;;;<sid>)`，管道设置 `PIPE_REJECT_REMOTE_CLIENTS` 且只允许一个实例。客户端以 `SECURITY_IMPERSONATION` 打开管道，因此服务可以模拟它。连接到达后，服务先模拟客户端，读取线程令牌 SID，并在调用驱动管理器之前恢复原令牌。无法模拟的连接会被直接关闭，不返回应答。不满足 DACL 的令牌在 `CreateFile` 处被拒绝。服务看不到这次调用，因此不审计它。管道 ACL 测试和服务控制管理器测试只在 Windows 上编译。

请求使用协议版本 1。未知字段、重复键、不属于 `strata-fs`、`strata-reg`、`strata-proc`、`strata-net` 的驱动，以及出现在错误操作上的驱动字段，返回 `invalid_record`。JSON 无法解析时返回 `invalid_json`。其他主版本返回 `unsupported_version`。声称的用户必须等于对端，且对端必须等于服务所有者，否则代码为 `identity_mismatch`。

审计日志保留 64 条。再写入一条时丢弃最旧记录并增加 `audit_dropped`。`generation`、`audit_dropped` 和 `faults` 以十进制字符串传输。宿主 panic 变成 `driver_fault`，`faults` 加一，随后的请求仍可执行。

## 驱动订阅

`Manager` 分别跟踪 `strata-fs`、`strata-reg`、`strata-proc` 和 `strata-net`。每个模块最多 64 个订阅。第一次成功加载进入 `ready`，并使 `generation` 加一。之后的订阅不再加载。最后一次释放先调用卸载，再改变计数，因此宿主 panic 时计数保持原值。卸载失败把计数置零，阶段变为 `unload_failed`。下一次订阅不再加载；因为映像可能仍驻留，模块回到 `ready`。`fault` 清除计数。驻留模块无法卸载时，阶段保持 `unload_failed`。一个模块的故障不改变其他模块。

`CatalogHost` 是 `listen_catalog` 使用的宿主。它的加载和卸载都返回 `image_absent`。它不调用服务控制管理器，也不改变驱动签名、Secure Boot 或 HVCI。用 `Privilege::WindowsElevated` 构造 `Session` 的单元测试只检查决策，不是已提升的 Windows 进程。这些测试中的 `ready` 只表示测试宿主返回了成功。

在 Windows 主机上，服务控制管理器测试会枚举现有驱动服务，为不存在的文件 `C:\Windows\Temp\WinStrata008-missing.sys` 创建 `WinStrata008MissingImage`，要求 `StartService` 以 Win32 错误 2 或 3 失败，然后删除该服务。若名称已存在，只有 `QueryServiceConfig` 报告同一映像时才删除。受限令牌必须在非受限打开已经成功之后，无法以创建权限打开管理器。另一项 Windows 测试要求同类受限令牌打开服务管道时得到拒绝访问，而普通客户端仍能查询状态。这些测试不启动已存在的文件，也不关闭签名检查。

`winstrata-service` 可执行文件监听默认端点，从标准输入读取一个字节后关闭。它不会被安装为 Windows 服务。

## 验证边界

Linux 运行了管理器和服务测试。在修正后的提交上，Windows Server 2022 通过了管理器测试、三项服务控制管理器测试、真实管道交换，以及受限令牌对 SCM 创建权限和管道的拒绝。没有构建或加载已签名的 INF/CAT 包。采集仍属于 PR 009 至 PR 011。E2E-14 和 E2E-15 仍是后续场景。命令、计数和剩余缺口见[工程记录](../records/STATUS.md)。
